## 3D cellular automaton: "{{name}}"

The same idea as the Game of Life, lifted into a 3D box of cells. {{blurb}}

<chips>

A cell's neighbours live in a little 3 × 3 × 3 box around it — shown here as three layers. {{#moore}}This rule counts every cell that touches it, even just at a corner:{{/moore}}{{^moore}}This rule only counts the 6 cells that share a whole face with it:{{/moore}}

<layers>

{{#fades}}
When a cell dies here it doesn't vanish instantly — it fades through in-between states (and stops counting as a neighbour) before disappearing:

<fade>
{{/fades}}

New seeds are pushed up through the floor of the box wherever Game of Life cells touch the boundary below. Bright cubes are alive, darker ones are fading away.
