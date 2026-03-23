# Slang

## Motivation
I've been writing some embedded C code for a college course and in the next few
weeks I will be working on a final project. The programming itself will be trivial
as I've already developed the necessary 'register level abstractions' for operating
the peripherals. So, this seems good enough an excuse to brush up on language design 
and implement the full compilation pipeline, from scratch, with no dependencies.

## Features
Here is a list of the features I have identified as _necessary_ for completing my
project.
- [x] Functions
- [ ] Arithmetic
    - [x] Bitwise 
    - [x] Signed/Unsigned integer
    - [ ] Vector
- [ ] Pointers
- [x] Control flow (if, while) 
- [ ] Global data
- [x] Constants (folded into literals at compile time)
- [ ] Cortex-M33 codegen
- [x] Static type checking 
    - [ ] Casts

### Notes
Functions should be able to be defined locally, there needs to be a way to
stop the `Scope` leaking into it, including `Arg` allocation in the IR stage.

Naive constant folding involves a lossy transformation of the syntax tree,
which means that errors downstream may report about things that are not longer
present, but I think this is fine, since literals are folded after variable
resolution and type checking which are bound to catch almost all of the errors.
