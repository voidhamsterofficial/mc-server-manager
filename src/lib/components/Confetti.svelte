<script lang="ts">
  // A lightweight DOM confetti burst. Re-fires whenever `trigger` increments.

  interface Props {
    trigger: number;
  }

  let { trigger }: Props = $props();

  const PIECE_COUNT = 28;
  const COLORS = ["#5ebd3e", "#ffaa00", "#55ffff", "#ff5555", "#ffff55", "#a06a42"];

  interface Piece {
    left: number;
    delay: number;
    duration: number;
    color: string;
    drift: number;
    size: number;
  }

  function makePieces(): Piece[] {
    return Array.from({ length: PIECE_COUNT }, () => ({
      left: Math.random() * 100,
      delay: Math.random() * 0.25,
      duration: 1.1 + Math.random() * 0.9,
      color: COLORS[Math.floor(Math.random() * COLORS.length)],
      drift: (Math.random() - 0.5) * 160,
      size: 6 + Math.random() * 6,
    }));
  }

  const pieces = $derived(trigger > 0 ? makePieces() : []);
</script>

{#key trigger}
  {#if pieces.length > 0}
    <div class="confetti" aria-hidden="true">
      {#each pieces as piece}
        <i
          style:left="{piece.left}%"
          style:background={piece.color}
          style:width="{piece.size}px"
          style:height="{piece.size * 0.6}px"
          style:animation-delay="{piece.delay}s"
          style:animation-duration="{piece.duration}s"
          style:--drift="{piece.drift}px"
        ></i>
      {/each}
    </div>
  {/if}
{/key}

<style>
  .confetti {
    position: fixed;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
    z-index: 200;
  }

  i {
    position: absolute;
    top: -12px;
    border-radius: 2px;
    animation-name: drop;
    animation-timing-function: ease-in;
    animation-fill-mode: forwards;
  }

  @keyframes drop {
    0% {
      transform: translate(0, 0) rotate(0deg);
      opacity: 1;
    }
    100% {
      transform: translate(var(--drift), 105vh) rotate(540deg);
      opacity: 0.4;
    }
  }
</style>
