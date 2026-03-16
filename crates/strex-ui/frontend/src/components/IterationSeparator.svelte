<script lang="ts">
  interface Props {
    iteration: number
    row: Record<string, string>
  }

  let { iteration, row }: Props = $props()

  let rowLabel = $derived(
    Object.keys(row)
      .sort()
      .map((k) => `${k}=${row[k]}`)
      .join(', ')
  )
</script>

<div class="iteration-separator">
  <span class="label">Iteration {iteration}</span>
  {#if rowLabel}
    <span class="dash">—</span>
    <span class="row-data">{rowLabel}</span>
  {/if}
  <span class="line"></span>
</div>

<style>
  .iteration-separator {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 20px 6px;
    font-size: 0.75rem;
    color: #666;
    user-select: none;
  }

  .label {
    font-weight: 700;
    color: #888;
    white-space: nowrap;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .dash {
    color: #444;
  }

  .row-data {
    color: #7b7baa;
    font-family: 'SF Mono', 'Fira Code', 'Fira Mono', 'Roboto Mono', monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 500px;
  }

  .line {
    flex: 1;
    height: 1px;
    background: #1e1e3a;
    min-width: 20px;
  }
</style>
