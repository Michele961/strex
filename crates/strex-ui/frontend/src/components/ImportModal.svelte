<script lang="ts">
  import { importGenerate, importSave } from '../lib/api'

  interface Props {
    onSaved: (filename: string) => void
    onClose: () => void
  }

  let { onSaved, onClose }: Props = $props()

  type Step = 'source-select' | 'input-form' | 'preview'
  type Source = 'curl' | 'openapi'
  type Mode = 'scaffold' | 'with_tests'

  let step = $state<Step>('source-select')
  let source = $state<Source>('curl')
  let input = $state('')
  let mode = $state<Mode>('scaffold')
  let generatedYaml = $state('')
  let filename = $state('imported-collection.yaml')
  let generating = $state(false)
  let saving = $state(false)
  let error = $state<string | null>(null)

  function selectSource(s: Source) {
    source = s
    step = 'input-form'
    error = null
  }

  async function handleGenerate() {
    if (!input.trim()) {
      error = source === 'curl' ? 'Paste a curl command first.' : 'Enter a file path or URL.'
      return
    }
    generating = true
    error = null
    try {
      const result = await importGenerate({ source, input: input.trim(), mode })
      generatedYaml = result.yaml
      step = 'preview'
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      generating = false
    }
  }

  function handleBack() {
    generatedYaml = ''
    step = 'input-form'
    error = null
  }

  async function handleSave() {
    if (!filename.trim()) {
      error = 'Filename is required.'
      return
    }
    saving = true
    error = null
    try {
      const result = await importSave({ yaml: generatedYaml, filename: filename.trim() })
      onSaved(result.filename)
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      saving = false
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose()
  }
</script>

<!-- Backdrop -->
<div class="backdrop" onclick={handleBackdropClick} role="dialog" aria-modal="true">
  <div class="modal">
    <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>

    {#if step === 'source-select'}
      <h2 class="modal-title">Import Collection</h2>
      <p class="modal-subtitle">Generate a Strex YAML collection from an existing source</p>

      <div class="source-list">
        <button class="source-tile" onclick={() => selectSource('curl')}>
          <span class="tile-name">curl command</span>
          <span class="tile-desc">Paste a curl snippet</span>
        </button>
        <button class="source-tile" onclick={() => selectSource('openapi')}>
          <span class="tile-name">OpenAPI / Swagger</span>
          <span class="tile-desc">File path or URL</span>
        </button>
        <button class="source-tile" disabled>
          <span class="tile-name">Postman collection</span>
          <span class="tile-desc">Coming soon</span>
        </button>
      </div>

    {:else if step === 'input-form'}
      <button class="back-link" onclick={() => { step = 'source-select'; error = null }}>← Back</button>
      <h2 class="modal-title">{source === 'curl' ? 'curl command' : 'OpenAPI / Swagger'}</h2>

      {#if source === 'curl'}
        <label class="field">
          <span>Paste your curl command</span>
          <textarea
            class="code-input"
            placeholder={"curl -X POST https://api.example.com/users -H 'Authorization: Bearer ...' -d '{...}'"}
            bind:value={input}
            rows={5}
          ></textarea>
        </label>
      {:else}
        <label class="field">
          <span>File path or URL</span>
          <input
            class="text-input"
            type="text"
            placeholder="./openapi.yaml  or  https://api.example.com/openapi.json"
            bind:value={input}
          />
        </label>
      {/if}

      <fieldset class="mode-toggle">
        <legend>Output mode</legend>
        <label class="mode-option">
          <input type="radio" bind:group={mode} value="scaffold" />
          Quick scaffold
        </label>
        <label class="mode-option">
          <input type="radio" bind:group={mode} value="with_tests" />
          Generate tests
        </label>
      </fieldset>

      {#if error}
        <p class="error-msg">{error}</p>
      {/if}

      <button class="primary-btn" onclick={handleGenerate} disabled={generating}>
        {generating ? 'Generating…' : 'Generate →'}
      </button>

    {:else if step === 'preview'}
      <button class="back-link" onclick={handleBack}>← Back</button>
      <h2 class="modal-title">Preview</h2>

      <pre class="yaml-preview">{generatedYaml}</pre>

      <label class="field">
        <span>Save as</span>
        <input class="text-input" type="text" bind:value={filename} />
      </label>

      {#if error}
        <p class="error-msg">{error}</p>
      {/if}

      <button class="primary-btn" onclick={handleSave} disabled={saving}>
        {saving ? 'Saving…' : 'Save'}
      </button>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-radius: 10px;
    padding: 28px;
    width: min(520px, 92vw);
    max-height: 90vh;
    overflow-y: auto;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 16px;
    color: #e0e0e0;
  }

  .close-btn {
    position: absolute;
    top: 14px;
    right: 16px;
    background: none;
    border: none;
    color: #666;
    font-size: 1rem;
    cursor: pointer;
    line-height: 1;
  }
  .close-btn:hover { color: #fff; }

  .modal-title {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    color: #e0e0e0;
  }

  .modal-subtitle {
    margin: 0;
    font-size: 0.8rem;
    color: #666;
  }

  .back-link {
    background: none;
    border: none;
    color: #ff6b35;
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0;
    text-align: left;
  }

  .source-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .source-tile {
    background: #0f0f23;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    padding: 12px 16px;
    text-align: left;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 2px;
    transition: border-color 0.15s;
  }
  .source-tile:hover:not(:disabled) { border-color: #ff6b35; }
  .source-tile:disabled { opacity: 0.4; cursor: not-allowed; }

  .tile-name { color: #e0e0e0; font-weight: 600; font-size: 0.9rem; }
  .tile-desc { color: #888; font-size: 0.78rem; }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.85rem;
    color: #bbb;
  }

  .code-input, .text-input {
    background: #0f0f23;
    border: 1px solid #333;
    border-radius: 4px;
    color: #e0e0e0;
    padding: 8px 10px;
    font-size: 0.82rem;
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
  }
  .code-input { font-family: monospace; }

  .mode-toggle {
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    padding: 10px 14px;
    display: flex;
    gap: 20px;
  }
  .mode-toggle legend { color: #888; font-size: 0.8rem; padding: 0 4px; }
  .mode-option { display: flex; align-items: center; gap: 6px; font-size: 0.85rem; color: #ccc; cursor: pointer; }

  .yaml-preview {
    background: #0f0f23;
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    padding: 12px;
    font-size: 0.75rem;
    font-family: monospace;
    color: #ccc;
    max-height: 240px;
    overflow-y: auto;
    white-space: pre;
    margin: 0;
  }

  .primary-btn {
    padding: 10px 16px;
    background: #ff6b35;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }
  .primary-btn:hover:not(:disabled) { background: #ff8555; }
  .primary-btn:disabled { background: #444; cursor: not-allowed; }

  .error-msg {
    margin: 0;
    font-size: 0.8rem;
    color: #f87171;
  }
</style>