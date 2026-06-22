## Wolfram's Rule 30

A one-dimensional world: a single row of cells, where each cell is either on or off

<single row>

Every tick the next generation is computed beneath the previous

<two rows>

A new cell is either on or off depending on exactly three cells from the previous generation: directly above, top-left, and top-right. There's only 8 possible rules:

<eight cases>

Those eight rules produce astonishingly complex behaviour, a phenomenon known as emergence

<forty steps>

Whilst the resulting pyramid is well behaved down the left edge, the rest of the structure has no discernible pattern whatsoever. No one has been able to predict what will happen ahead of time, the best we can do is compute it and see. This emergent behaviour has made rule 30 a source of fascination for many, and has seen it used as a [random number generator in Mathematica](https://reference.wolfram.com/language/tutorial/RandomNumberGeneration.html) as well as being proposed as a [cryptographic stream-cipher](https://link.springer.com/chapter/10.1007/3-540-39799-X_32)

## Determinism vs. Predictability

<laplace>

In 1814, Laplace articulated a vision of determinism which many found uncomfortable. He imagined an intellect (known today as Laplace’s Demon) who knew the exact position and momentum of every particle in the Universe. Armed with this and the laws of physics, the demon could perfectly predict the infinite future. Laplace's demon rendered the universe a clock-like machine.

A little over a century later Quantum Mechanics doomed the demon to fundamental ignorance. Heisenberg's Uncertainty Principle states that the more precisely the demon knew a particle's position, the less it could know of its momentum (and vice versa). Unbeknownst to Laplace, the clock-like machine had probabilistic uncertainty built in.

But Rule 30 offers us a different, quieter truth: there's no quantum uncertainty in Rule 30, just eight simple laws. It's perfectly deterministic, yet remains perfectly unpredictable.
