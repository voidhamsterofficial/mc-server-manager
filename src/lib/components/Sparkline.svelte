<script lang="ts">
  // Single-series sparkline: 2px line + soft area fill, no axes (the tile's
  // label and value carry the identity and current reading).

  interface Props {
    values: number[];
    /** Fixed scale maximum; omit to autoscale to the data. */
    max?: number;
    color: string;
    height?: number;
  }

  let { values, max, color, height = 44 }: Props = $props();

  const VIEW_WIDTH = 200;

  const scaleMax = $derived(max ?? Math.max(...values, 1) * 1.1);

  const points = $derived(buildPoints(values, scaleMax));
  const areaPath = $derived(
    points === "" ? "" : `M0,${height} L${points.replaceAll(" ", " L")} L${VIEW_WIDTH},${height} Z`,
  );

  function buildPoints(series: number[], top: number): string {
    if (series.length < 2) {
      return "";
    }
    const step = VIEW_WIDTH / (series.length - 1);
    const coordinates = series.map((value, index) => {
      const x = (index * step).toFixed(1);
      const clamped = Math.min(Math.max(value / top, 0), 1);
      const y = (height - clamped * (height - 4)).toFixed(1);
      return `${x},${y}`;
    });
    return coordinates.join(" ");
  }
</script>

<svg
  viewBox="0 0 {VIEW_WIDTH} {height}"
  preserveAspectRatio="none"
  style:height="{height}px"
  aria-hidden="true"
>
  {#if points !== ""}
    <path d={areaPath} fill={color} opacity="0.12" />
    <polyline
      points={points}
      fill="none"
      stroke={color}
      stroke-width="2"
      stroke-linejoin="round"
      stroke-linecap="round"
      vector-effect="non-scaling-stroke"
    />
  {/if}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
  }
</style>
