<script lang="ts">
  interface Props {
    /** 0..1, or null for indeterminate. */
    value: number | null;
  }

  let { value }: Props = $props();

  const percent = $derived(value === null ? 100 : Math.min(100, Math.max(0, value * 100)));
</script>

<div class="track" role="progressbar" aria-valuenow={value === null ? undefined : percent}>
  <div class="fill" class:indeterminate={value === null} style:width="{percent}%"></div>
</div>

<style>
  /* XP-bar style: dark chunky track, bright green fill with pixel notches. */
  .track {
    height: 14px;
    border-radius: 5px;
    background: #1d1926;
    box-shadow: inset 0 0 0 2px rgba(20, 12, 38, 0.6);
    overflow: hidden;
    padding: 3px;
  }

  .fill {
    height: 100%;
    border-radius: 3px;
    background:
      repeating-linear-gradient(
        90deg,
        transparent 0 6px,
        rgba(0, 0, 0, 0.16) 6px 8px
      ),
      linear-gradient(180deg, #8ef05c 0%, #62c93a 55%, #3f9c22 100%);
    transition: width 0.25s ease;
  }

  .fill.indeterminate {
    animation: slide 1.4s ease-in-out infinite;
  }

  @keyframes slide {
    0%,
    100% {
      transform: translateX(-30%);
    }
    50% {
      transform: translateX(30%);
    }
  }
</style>
