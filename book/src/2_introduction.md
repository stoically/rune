# Introduction

Welcome the *The Rune Programming Language*, a reference guide that will
familiarize yourself with Rune.

Rune is an embeddable dynamic programming language that compiles and runs on a
virtual machine called Runestick (thanks Brendan).

The goal of Rune is to reimagine Rust as a dynamic programming language. Trying
to mimic as many concepts as possible, and remixing the ones which do not
translate directly. We do this by using the same syntax as Rust. But a few
additions are inevitable because certain things are just done differently when
you have a dynamic environment.

I also concede that a number of program correctness features you get through
static typing will be sorely lacking. The tradeoff you get for this are fast
compilation times and *duck typing*, sometimes leading to more concise and
compact code. [Python] is a great example of this, and is along with [Rhai] and
[Lua] biggest inspirations for this project.

[Python]: https://python.org
[Rhai]: https://github.com/jonathandturner/rhai
[Lua]: http://www.lua.org/

With that out of the way, let's get started. We have a bit to go through.