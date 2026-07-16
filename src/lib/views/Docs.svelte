<script lang="ts">
  import { fade } from "svelte/transition";

  interface Props {
    /** Navigate to another app page — docs links are real links. */
    onopenview: (view: "home" | "settings") => void;
  }

  let { onopenview }: Props = $props();

  type LinkTarget = { kind: "view"; view: "home" | "settings" } | { kind: "topic"; topicId: string };

  interface Segment {
    text: string;
    link?: LinkTarget;
  }

  interface Topic {
    id: string;
    emoji: string;
    title: string;
    paragraphs: Segment[][];
  }

  const TOPICS: Topic[] = [
    {
      id: "getting-started",
      emoji: "🌱",
      title: "Getting started",
      paragraphs: [
        [
          { text: "Create your first server from the " },
          { text: "dashboard", link: { kind: "view", view: "home" } },
          {
            text: " with the ＋ New server button. Pick your software (Vanilla, Paper, Fabric, a proxy, …), then a version, port, and memory. Blockparty downloads everything — including ",
          },
          { text: "the right Java", link: { kind: "topic", topicId: "java" } },
          { text: " — and you're ready to press Start." },
        ],
        [
          {
            text: "Servers live in your Documents folder by default; change that in ",
          },
          { text: "Settings", link: { kind: "view", view: "settings" } },
          { text: ", or per server when creating it." },
        ],
      ],
    },
    {
      id: "software",
      emoji: "🧱",
      title: "Server software",
      paragraphs: [
        [
          {
            text: "Every major server type installs automatically: Vanilla, the Paper family (Paper, Purpur, Folia), mod loaders (Fabric, Quilt, Forge, NeoForge), hybrids (Mohist, Arclight), Bedrock, and the Velocity/BungeeCord proxies.",
          },
        ],
        [
          {
            text: "A few take longer than a download: Spigot is compiled from source with BuildTools (several minutes), and Forge/NeoForge run their official installer after downloading. Bedrock is a native server — no Java at all — available on Windows and Linux.",
          },
        ],
        [
          { text: "Pick your software on the first screen of the " },
          { text: "new-server wizard", link: { kind: "view", view: "home" } },
          { text: "; each type shows its own supported version list." },
        ],
      ],
    },
    {
      id: "java",
      emoji: "☕",
      title: "Java, automatically",
      paragraphs: [
        [
          {
            text: "Every Minecraft version needs a matching Java. Blockparty detects the Javas installed on your machine and picks the right one; if none fits, it downloads a Temurin JRE by itself — you'll see a ☕ pill while that happens.",
          },
        ],
        [
          { text: "See what was found under " },
          { text: "Settings → Java installations", link: { kind: "view", view: "settings" } },
          {
            text: ". Power users can add JVM flags or a fully custom start command in the wizard's Advanced section.",
          },
        ],
      ],
    },
    {
      id: "players",
      emoji: "🧑‍🤝‍🧑",
      title: "Players & moderation",
      paragraphs: [
        [
          {
            text: "The Players tab manages anyone by name (whitelist, pardon, ban — works offline) and shows everyone online with one-click Op, Kick, and Ban.",
          },
        ],
        [
          {
            text: "Player history at the bottom remembers everyone who ever joined: playtime, join and kick counts, and live ban status straight from the server's banned-players.json.",
          },
        ],
      ],
    },
    {
      id: "backups",
      emoji: "🎁",
      title: "Backups & restores",
      paragraphs: [
        [
          {
            text: "Backups zip the whole server folder (a running server is flushed with save-all first) into a backups folder next to the world — configurable per server in its Settings tab. Restores replace everything except the backups themselves.",
          },
        ],
        [
          { text: "Automate them with the " },
          { text: "scheduler", link: { kind: "topic", topicId: "scheduler" } },
          { text: " — a nightly backup task takes about ten seconds to set up." },
        ],
      ],
    },
    {
      id: "scheduler",
      emoji: "⏰",
      title: "Scheduler",
      paragraphs: [
        [
          {
            text: "Each server's Scheduler tab runs commands, restarts, backups, starts, and stops on a schedule — presets for common cadences, raw cron for full control, with a next-run preview.",
          },
        ],
        [
          {
            text: "Tasks run while Blockparty is open. A disabled task keeps its schedule but skips firing.",
          },
        ],
      ],
    },
    {
      id: "recovery",
      emoji: "🧯",
      title: "When things go wrong",
      paragraphs: [
        [
          {
            text: "\"The process cannot access the file…\" means an old server process is still holding the world. Blockparty reclaims these automatically on start, and ",
          },
          {
            text: "Settings → Recovery",
            link: { kind: "view", view: "settings" },
          },
          {
            text: " has a kill-switch for every server process Blockparty is responsible for — your game and launcher are never touched.",
          },
        ],
        [
          { text: "A crashed server turns red — check the last lines of its Console tab, then " },
          { text: "restore a backup", link: { kind: "topic", topicId: "backups" } },
          { text: " if the world itself is damaged." },
        ],
      ],
    },
  ];

  let activeTopicId = $state(TOPICS[0].id);

  const activeTopic = $derived(
    TOPICS.find((topic) => topic.id === activeTopicId) ?? TOPICS[0],
  );

  function followLink(link: LinkTarget) {
    if (link.kind === "view") {
      onopenview(link.view);
      return;
    }
    activeTopicId = link.topicId;
  }
</script>

<section class="docs" in:fade={{ duration: 120 }}>
  <h1>Docs 📖</h1>

  <div class="layout">
    <nav class="topics">
      {#each TOPICS as topic (topic.id)}
        <button
          class="topic"
          class:active={topic.id === activeTopicId}
          onclick={() => (activeTopicId = topic.id)}
        >
          <span>{topic.emoji}</span>
          {topic.title}
        </button>
      {/each}
    </nav>

    <article class="content">
      <h3>{activeTopic.emoji} {activeTopic.title}</h3>
      {#each activeTopic.paragraphs as paragraph, index (index)}
        <p>
          {#each paragraph as segment, segmentIndex (segmentIndex)}
            {#if segment.link}
              {@const link = segment.link}
              <button class="doc-link" onclick={() => followLink(link)}>
                {segment.text}
              </button>
            {:else}
              {segment.text}
            {/if}
          {/each}
        </p>
      {/each}
    </article>
  </div>
</section>

<style>
  .docs {
    max-width: 960px;
    margin: 0 auto;
    padding: 1.5rem 2rem 3rem;
  }

  h1 {
    font-size: 1.5rem;
    margin: 0 0 1rem;
  }

  .layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 1rem;
    align-items: start;
  }

  .topics {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .topic {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border: none;
    background: transparent;
    color: var(--text);
    font-family: inherit;
    font-size: 0.92rem;
    font-weight: 600;
    text-align: left;
    padding: 0.55rem 0.7rem;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .topic:hover {
    background: var(--surface-2);
  }

  .topic.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .content {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1.25rem 1.5rem;
    min-height: 300px;
  }

  .content h3 {
    margin: 0 0 0.75rem;
    font-size: 1.05rem;
  }

  .content p {
    margin: 0 0 0.8rem;
    font-size: 0.92rem;
    line-height: 1.65;
    color: var(--text);
  }

  /* In-app links: real navigation, styled like links. */
  .doc-link {
    display: inline;
    border: none;
    background: transparent;
    padding: 0;
    font: inherit;
    color: var(--accent-strong);
    font-weight: 700;
    text-decoration: underline;
    cursor: pointer;
  }

  .doc-link:hover {
    color: var(--accent);
  }

  @media (max-width: 760px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
</style>
