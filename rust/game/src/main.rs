//! Liero-rs `game` binary — the project's first Bevy code (Slice 3c).
//! T1: an empty 960×600 window that closes on Esc / the close button. The sim
//! wiring lands in T3. Bevy 0.19; the ONLY Bevy crate in the workspace.
use bevy::prelude::*;
use bevy::window::WindowResolution;

mod blit;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                // Global nearest-neighbor sampling for crisp integer upscaling.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: WindowResolution::new(960, 600),
                        title: "Liero-rs — 3c demo".into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Update, close_on_esc)
        .run();
}

/// Esc quits. The window's close button already exits via winit.
///
/// Bevy 0.19 renamed the buffered-event API to "messages": the exit signal is
/// sent through a `MessageWriter<AppExit>` (`EventWriter` no longer exists;
/// `AppExit` derives `Message`, and `MessageWriter::write` is the send call).
fn close_on_esc(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
