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
  .track {
    height: 10px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent), var(--lavender), var(--accent));
    background-size: 200% 100%;
    animation: shimmer 1.6s linear infinite;
    transition: width 0.25s ease;
  }

  .fill.indeterminate {
    animation:
      shimmer 1.6s linear infinite,
      slide 1.4s ease-in-out infinite;
  }

  @keyframes shimmer {
    from {
      background-position: 0% 0;
    }
    to {
      background-position: 200% 0;
    }
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
