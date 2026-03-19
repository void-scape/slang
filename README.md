# Slang

## Motivation
I've been writing some embedded C code for a college course and in the next few
weeks I will be working on a final project. The programming itself will be trivial
as I've already developed the necessary 'register level abstractions' for operating
the peripherals. So, this seems good enough an excuse to brush up on language design 
and implement the full compilation pipeline, from scratch, with no dependencies.

## Features
Here is a list of the features I have identified as _necessary_ for completing my
project. There are several, very important features in any self respecting lanugage
that are _not_ considered in this list.
[_] Static type checking 
[_] Pointers
[_] Casts
[_] Functions
[_] Global data
[_] Arithmetic
    [_] Signed/Unsigned integer
    [_] Vector
[_] Bitwise operations 
[_] Control flow (if, while, for) 
[_] Cortex-M33 codegen

### TODO
- Data returned from function calls needs to be stored on the stack, which means that the parser is going to need to do a prepass to collect all of the function signatures in advance.
