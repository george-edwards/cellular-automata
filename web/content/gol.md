## Conway's Game of Life

A two-dimensional world being fed input from below by the oldest row of Rule 30.

A cell's state in the Game of Life is decided by how many of its neighbours are alive.

<rules>

Released in 1970 by mathematician John Conway, the Game of Life has captivated generations of programmers, artists, and curious minds alike.

<conway>

Over the past fifty years, people have found and catalogued a diverse taxonomy of emergent structures, including:

[Oscillators](https://conwaylife.com/wiki/Oscillator): patterns that cycle through a fixed set of states

<oscillator>

[Gliders](https://conwaylife.com/wiki/glider): Oscillators that translate while cycling, crawling across the grid

<glider>

[Guns](https://conwaylife.com/wiki/Gun): periodically emit gliders

<gun>

[And](https://conwaylife.com/wiki/Garden_of_Eden), [many](https://conwaylife.com/wiki/Eater), [more](https://conwaylife.com/wiki/Pufferfish).

## Turing Completeness

In 1936, Alan Turing proposed an absurdly stripped-down model of computation: the Turing Machine.

<alan-turing>

A Turing Machine is deliberately simple. It has an infinite strip of tape divided into cells (storage), from which a head can read and write symbols (data), and a list of instructions. Move left one cell. Write a symbol. Move right one cell. Etcetera.

<turing-machine>

The point was not to build a practical computer. The point was to make computation simple enough to reason about mathematically. With this toy machine, Turing reasoned fascinating truths about what can and cannot be computed. He found the limits of computation. He found its bounds.

Surprisingly, the Turing Machine can compute anything your modern laptop can, given enough time and tape. A Turing Machine can run Doom, albeit at a glacial pace and without a screen. To this day, no one has discovered a more powerful system of computation. Any system capable of matching this baseline power is what we call 'Turing Complete'.

**The Game of Life is Turing Complete**.

Over decades, people found enough structures to build the components of a Turing complete system within the Game of Life. Gliders act as signals. Eaters absorb them. Carefully arranged collisions redirect them, duplicate, destroy, or transform them. From these components, logic gates are built: AND, OR, NOT, etc.

<gol-logic-gate>

The Game of Life can become a computer. Not metaphorically. Literally.

https://www.youtube.com/watch?v=xP5-iIeKXE8