use bytemuck::{Zeroable, Pod};
/// Marker component for end crystal entities.
///
/// # Example
/// A system that queries for all end crystals:
/// ```no_run
/// use quill::{Game, Position, entities::EndCrystal};
/// # struct MyPlugin;
/// fn print_entities_system(_plugin: &mut MyPlugin, game: &mut Game) {
///     for (entity, (position, _)) in game.query::<(&Position, &EndCrystal)>() {
///         println!("Found a end crystal with position {:?}", position);
///     }
/// }
/// ```
#[derive(Debug, Copy, Clone, Zeroable, Pod)]
#[repr(C)]
pub struct EndCrystal;

pod_component_impl!(EndCrystal);
