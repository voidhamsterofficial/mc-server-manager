<script lang="ts">
  // A little original pixel-art grass block (our own drawing, no game
  // assets) used as the app's mark.

  interface Props {
    size?: number;
  }

  let { size = 22 }: Props = $props();

  // 10x10 pixel grid: rows of [color, startX, width] runs.
  const PIXEL_RUNS: [string, number, number, number][] = [
    // Grass top (minty greens to match the palette)
    ["#7ed957", 0, 0, 10],
    ["#5cbf3f", 0, 1, 3],
    ["#7ed957", 3, 1, 4],
    ["#5cbf3f", 7, 1, 3],
    ["#4aa332", 0, 2, 1],
    ["#5cbf3f", 1, 2, 2],
    ["#4aa332", 3, 2, 2],
    ["#5cbf3f", 5, 2, 3],
    ["#4aa332", 8, 2, 2],
    // Dirt body (warm browns)
    ["#a06a42", 0, 3, 10],
    ["#8a5632", 0, 4, 2],
    ["#a06a42", 2, 4, 3],
    ["#8a5632", 5, 4, 2],
    ["#a06a42", 7, 4, 3],
    ["#96613a", 0, 5, 10],
    ["#8a5632", 2, 6, 2],
    ["#96613a", 0, 6, 2],
    ["#a06a42", 4, 6, 6],
    ["#8a5632", 0, 7, 10],
    ["#96613a", 0, 8, 4],
    ["#8a5632", 4, 8, 2],
    ["#96613a", 6, 8, 4],
    ["#7d4e2d", 0, 9, 10],
  ];
</script>

<svg
  viewBox="0 0 10 10"
  width={size}
  height={size}
  style:image-rendering="pixelated"
  shape-rendering="crispEdges"
  aria-hidden="true"
>
  {#each PIXEL_RUNS as [color, x, y, width], index (index)}
    <rect {x} {y} {width} height="1" fill={color} />
  {/each}
</svg>

<style>
  svg {
    display: block;
    border-radius: 3px;
  }
</style>
