# Slang

## Motivation
I've been writing some embedded C code for a college course and in the next few
weeks I will be working on a final project. The programming itself will be trivial
as I've already developed the necessary 'register level abstractions' for operating
the peripherals. So, this seems good enough an excuse to brush up on language design 
and implement the full compilation pipeline, from scratch, with no dependencies.

## Features
Here is a list of the features I have identified as _necessary_ for completing my
project. Funily enough, I actually don't really _need_ static type checking if a 
I have reasonable assertions in the IR, and because it is the most difficult 
feature I will consider it last.
- [x] Functions
- [/] Arithmetic
    - [x] Bitwise 
    - [/] Signed/Unsigned integer
    - [ ] Vector
- [ ] Pointers
- [x] Control flow (if, while) 
- [ ] Global data
- [ ] Cortex-M33 codegen
- [ ] Static type checking 
    - [ ] Casts
