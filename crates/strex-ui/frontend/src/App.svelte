<script lang="ts">
  let message = $state('strex UI loading...')
</script>

<div class="app">
  <p>{message}</p>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, sans-serif;
    background: #13132b;
    color: #e0e0e0;
  }

  .app {
    display: flex;
    height: 100vh;
    align-items: center;
    justify-content: center;
  }
</style>
