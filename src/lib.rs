#![feature(generators, generator_trait)]
#[macro_use]
extern crate vst;

use rand::random;
use std::os::raw::c_void;
use std::sync::Arc;
use vst::api::{Events, Supported};
use vst::buffer::AudioBuffer;
use vst::editor::Editor;
use vst::event::Event;
use vst::plugin::{CanDo, Category, Info, Plugin, PluginParameters};
use vst::util::AtomicFloat;

#[derive(Default)]
struct Whisper {
    params: Arc<WhisperParameters>,
    // Added a counter in our plugin struct.
    notes: u8,
}

struct WhisperParameters {
    volume: AtomicFloat,
}

impl Default for WhisperParameters {
    fn default() -> Self {
        Self {
            volume: AtomicFloat::new(1.0),
        }
    }
}

// We're implementing a trait `Plugin` that does all the VST-y stuff for us.
impl Plugin for Whisper {
    fn get_info(&self) -> Info {
        Info {
            name: "Whisper".to_string(),

            // Used by hosts to differentiate between plugins.
            unique_id: 1337,

            // We don't need inputs
            inputs: 0,

            // We do need two outputs though.  This is default, but let's be
            // explicit anyways.
            outputs: 2,

            // Set our category
            category: Category::Synth,

            parameters: 1,

            // We don't care about other stuff, and it can stay default.
            ..Default::default()
        }
    }

    // It's good to tell our host what our plugin can do.
    // Some VST hosts might not send any midi events to our plugin
    // if we don't explicitly tell them that the plugin can handle them.
    fn can_do(&self, can_do: CanDo) -> Supported {
        match can_do {
            // Tell our host that the plugin supports receiving MIDI messages
            CanDo::ReceiveMidiEvent => Supported::Yes,
            // Maybe it also supports ather things
            _ => Supported::Maybe,
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        // `buffer.split()` gives us a tuple containing the
        // input and output buffers.  We only care about the
        // output, so we can ignore the input by using `_`.
        let (_, mut output_buffer) = buffer.split();

        // We only want to process *anything* if a note is being held.
        // Else, we can fill the output buffer with silence.
        if self.notes == 0 {
            for output_channel in output_buffer.into_iter() {
                // Let's iterate over every sample in our channel.
                for output_sample in output_channel {
                    *output_sample = 0.0;
                }
            }
            return;
        }

        let volume = self.params.volume.get();

        // Now, we want to loop over our output channels.  This
        // includes our left and right channels (or more, if you
        // are working with surround sound).
        for output_channel in output_buffer.into_iter() {
            // Let's iterate over every sample in our channel.
            for output_sample in output_channel {
                // For every sample, we want to generate a random value
                // from -1.0 to 1.0.
                *output_sample = (random::<f32>() - 0.5f32) * 2f32 * volume;
            }
        }
    }

    // Here's the function that allows us to receive events
    fn process_events(&mut self, events: &Events) {
        // Some events aren't MIDI events - so let's do a match
        // to make sure we only get MIDI, since that's all we care about.
        for event in events.events() {
            match event {
                Event::Midi(ev) => {
                    // Check if it's a noteon or noteoff event.
                    // This is difficult to explain without knowing how the MIDI standard works.
                    // Basically, the first byte of data tells us if this signal is a note on event
                    // or a note off event.  You can read more about that here:
                    // https://www.midi.org/specifications/item/table-1-summary-of-midi-message
                    match ev.data[0] {
                        // if note on, increment our counter
                        144 => self.notes += 1u8,

                        // if note off, decrement our counter
                        128 => self.notes -= 1u8,
                        _ => (),
                    }
                    // if we cared about the pitch of the note, it's stored in `ev.data[1]`.
                }
                // We don't care if we get any other type of event
                _ => (),
            }
        }
    }

    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }

    fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
        Some(Box::new(GUIWrapper {
            inner: None,
            params: self.params.clone(),
        }))
    }
}

plugin_main!(Whisper);

impl PluginParameters for WhisperParameters {
    fn get_parameter_label(&self, index: i32) -> String {
        match index {
            0 => "x".to_string(),
            _ => "".to_string(),
        }
    }
    // This is what will display underneath our control.  We can
    // format it into a string that makes the most sense.
    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 => format!("{:.3}", self.volume.get()),
            _ => format!(""),
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "volume".to_string(),
            _ => "".to_string(),
        }
    }
    // get_parameter has to return the value used in set_parameter
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.volume.get(),
            _ => 0.0,
        }
    }
    fn set_parameter(&self, index: i32, value: f32) {
        match index {
            0 => self.volume.set(value),
            _ => (),
        }
    }
}

use iced_winit::Application;
use iced_winit::Command;
use winapi::shared::windef::HWND;

use std::ops::Generator;

const WIDTH: u32 = 400;
const HEIGHT: u32 = 200;

struct GUIWrapper {
    params: Arc<WhisperParameters>,
    inner: Option<GUI>,
}

struct GUI {
    gen: Box<dyn std::marker::Unpin + std::ops::Generator<Yield = (), Return = ()>>,
}

impl GUI {
    fn new(parent: HWND, params: Arc<WhisperParameters>) -> Self {
        let mut setting = iced_winit::Settings::default();
        setting.window.decorations = false;
        setting.window.platform_specific.parent = Some(parent);
        setting.window.size = (WIDTH, HEIGHT);

        let app = Counter::new(params);
        let gen = app.run_generator(Command::none(), setting);

        Self { gen }
    }
}

impl Editor for GUIWrapper {
    fn size(&self) -> (i32, i32) {
        (WIDTH as i32, HEIGHT as i32)
    }

    fn position(&self) -> (i32, i32) {
        (0, 0)
    }

    fn idle(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            if let std::ops::GeneratorState::Complete(_) =
                Generator::resume(std::pin::Pin::new(&mut inner.gen))
            {
                self.inner = None;
            }
        }
    }

    fn close(&mut self) {
        self.inner = None;
    }

    fn open(&mut self, parent: *mut c_void) -> bool {
        self.inner = Some(GUI::new(parent as HWND, self.params.clone()));
        true
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_some()
    }
}

use iced::{Column, Element, Text};

struct Counter {
    params: Arc<WhisperParameters>,
    volume_slider: iced::widget::slider::State,
}

impl Counter {
    fn new(params: Arc<WhisperParameters>) -> Self {
        Self {
            params,
            volume_slider: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Message {
    VolumeChanged(f32),
}

impl iced_winit::Application for Counter {
    type Renderer = iced_wgpu::Renderer;
    type Message = Message;

    fn new() -> (Self, Command<Self::Message>) {
        // (Self::default(), Command::none())
        unimplemented!()
    }

    fn title(&self) -> String {
        String::from("A simple counter")
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::VolumeChanged(v) => {
                self.params.volume.set(v);
            }
        }
        Command::none()
    }

    fn view(&mut self) -> Element<Message> {
        Column::new()
            .padding(20)
            .push(Text::new("Volume".to_string()).size(32))
            .push(iced::widget::Slider::new(
                &mut self.volume_slider,
                0.0..=1.0,
                self.params.volume.get(),
                Message::VolumeChanged,
            ))
            .into()
    }
}
