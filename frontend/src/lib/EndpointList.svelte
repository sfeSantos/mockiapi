<script lang="ts">
    import type { Endpoint } from './types';

    interface Props {
        endpoints: Endpoint[];
        loading: boolean;
        ondelete: (path: string) => void;
    }

    let { endpoints, loading, ondelete }: Props = $props();
</script>

<section class="registered-endpoints">
    <h2>📋 Registered Endpoints</h2>

    {#if loading}
        <div class="loader"></div>
    {/if}

    {#if endpoints.length === 0}
        <div class="empty-state">No endpoints registered yet.</div>
    {:else}
        <table>
            <thead>
                <tr>
                    <th>Type</th>
                    <th>Path</th>
                    <th>Methods</th>
                    <th>Status</th>
                    <th>Auth</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                {#each endpoints as endpoint (endpoint.path)}
                    <tr>
                        <td>{endpoint.type}</td>
                        <td>{endpoint.path}</td>
                        <td>
                            <div class="methods-list">
                                {#each endpoint.method as method}
                                    <span class="method-tag">{method}</span>
                                {/each}
                            </div>
                        </td>
                        <td>{endpoint.status_code}</td>
                        <td>{endpoint.authentication ? 'Yes' : 'No'}</td>
                        <td>
                            <button class="btn-danger" type="button" onclick={() => ondelete(endpoint.path)}>
                                Delete
                            </button>
                        </td>
                    </tr>
                {/each}
            </tbody>
        </table>
    {/if}
</section>
