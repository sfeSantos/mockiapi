<script lang="ts">
    import { summaryOf } from './summary';
    import type { Endpoint } from './types';

    interface Props {
        endpoints: Endpoint[];
    }

    let { endpoints }: Props = $props();

    const summary = $derived(summaryOf(endpoints));
</script>

<section class="summary" aria-label="Summary">
    <dl class="stats">
        <div class="stat">
            <dt>Endpoints</dt>
            <dd>{summary.endpoints}</dd>
        </div>
        <div class="stat">
            <dt>Groups</dt>
            <dd>{summary.groups}</dd>
        </div>
        <div class="stat" title="Endpoints that ask for Basic auth or a bearer token">
            <dt>Protected</dt>
            <dd>{summary.protected}</dd>
        </div>
        <div class="stat" title="Endpoints that answer after a delay or under a rate limit">
            <dt>With delay or limit</dt>
            <dd>{summary.shaped}</dd>
        </div>
    </dl>

    <div class="summary-notes">
        {#if summary.kinds.length > 0}
            <ul class="kinds">
                {#each summary.kinds as { kind, count }}
                    <li><strong>{count}</strong> {kind}</li>
                {/each}
            </ul>
        {/if}
        <p>Endpoints are kept in memory until the server stops.</p>
    </div>
</section>
