<script lang="ts">
    import type { Endpoint } from './types';

    interface Props {
        endpoints: Endpoint[];
        /** The registered path of the endpoint open in the editor, if any. */
        selectedPath: string | null;
        loading: boolean;
        onselect: (path: string) => void;
    }

    let { endpoints, selectedPath, loading, onselect }: Props = $props();
</script>

<nav class="endpoint-nav" aria-label="Registered endpoints">
    <div class="pane-heading">
        <span>Registered Endpoints</span>
        {#if loading}
            <span class="loader" role="status" aria-label="Loading"></span>
        {:else}
            <span class="count">{endpoints.length}</span>
        {/if}
    </div>

    {#if endpoints.length === 0}
        <p class="empty-state">No endpoints registered yet.</p>
    {:else}
        <ul>
            {#each endpoints as endpoint (endpoint.path)}
                <li>
                    <button
                        type="button"
                        class="endpoint-item"
                        class:selected={endpoint.path === selectedPath}
                        aria-current={endpoint.path === selectedPath ? 'true' : undefined}
                        onclick={() => onselect(endpoint.path)}
                    >
                        <span class="methods">
                            {#each endpoint.method as method}
                                <span class="method method-{method.toLowerCase()}">{method}</span>
                            {/each}
                        </span>
                        <span class="path" title={endpoint.path}>{endpoint.path}</span>
                        {#if endpoint.type !== 'REST'}
                            <span class="kind">{endpoint.type}</span>
                        {/if}
                    </button>
                </li>
            {/each}
        </ul>
    {/if}
</nav>
