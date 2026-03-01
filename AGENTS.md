This is here for LLM-based agents so that they do not make the same mistakes multiple times.

There is no such thing as `bevy::ecs::system::entity_command::despawn_recursive()`.
Use `bevy::ecs::system::entity_command::despawn()` instead. This will also despawn the entities in any RelationshipTarget that is configured to despawn descendants. For example, this will recursively despawn Children.
