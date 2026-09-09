# Maintainability baseline

Supplied to the `maintainability` reviewer only, under `--quality`. A documented repository standard always overrides an entry here, and anything tooling already enforces is skipped. Every entry is a labeled judgment call, never a hard violation, and findings stay inside the epic diff.

- **Mysterious Name**: a name that does not reveal what it does or holds. Rename it; if no honest name comes, the design is murky.
- **Duplicated Code**: the same logic shape in more than one place in the change. Extract it, call it from both.
- **Feature Envy**: a function reaching into another object's data more than its own. Move it onto the data it envies.
- **Data Clumps**: the same few fields or params always traveling together. Bundle them into one type.
- **Primitive Obsession**: a primitive or string standing in for a domain concept. Give the concept its own small type.
- **Repeated Switches**: the same cascade on the same type recurring across the change. Replace with polymorphism or one shared map.
- **Shotgun Surgery**: one logical change forcing scattered edits across many files. Gather what changes together.
- **Divergent Change**: one module edited for several unrelated reasons. Split it so each changes for one reason.
- **Speculative Generality**: abstraction, params, or hooks added for needs the PRD does not have. Delete it.
- **Message Chains**: long navigation the caller should not depend on. Hide the walk behind one method.
- **Middle Man**: a unit that mostly delegates onward. Cut it, call the real target directly.
- **Refused Bequest**: an implementer ignoring or overriding most of what it inherits. Use composition.
