<script lang="ts">
    import { GROUPINGS, GROUPING_LABELS, sectionsOf, storedGrouping, storeGrouping } from './grouping';
    import type { Grouping } from './grouping';
    import type { Endpoint } from './types';

    interface Props {
        endpoints: Endpoint[];
        /** The id of the endpoint open in the editor, if any. */
        selectedId: number | null;
        loading: boolean;
        onselect: (id: number) => void;
    }

    let { endpoints, selectedId, loading, onselect }: Props = $props();

    let grouping = $state<Grouping>(storedGrouping());

    const sections = $derived(sectionsOf(endpoints, grouping));

    function groupBy(choice: Grouping) {
        grouping = choice;
        storeGrouping(choice);
    }
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
        <div class="grouping">
            <span id="grouping-label">Group by</span>
            <div class="segmented" role="group" aria-labelledby="grouping-label">
                {#each GROUPINGS as choice}
                    <button
                        type="button"
                        class:active={grouping === choice}
                        aria-pressed={grouping === choice}
                        onclick={() => groupBy(choice)}
                    >
                        {GROUPING_LABELS[choice]}
                    </button>
                {/each}
            </div>
        </div>

        {#each sections as section (section.title)}
            {#if section.title !== null}
                <h2 class="section-title">
                    <span>{section.title}</span>
                    <span class="count">{section.endpoints.length}</span>
                </h2>
            {/if}

            <ul>
                {#each section.endpoints as endpoint (endpoint.id)}
                    <li>
                        <button
                            type="button"
                            class="endpoint-item"
                            class:selected={endpoint.id === selectedId}
                            aria-current={endpoint.id === selectedId ? 'true' : undefined}
                            onclick={() => onselect(endpoint.id)}
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
        {/each}
    {/if}
</nav>
