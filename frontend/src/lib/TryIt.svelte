<script lang="ts">
    import { untrack } from 'svelte';
    import { authorizationFor, curlFor, send, trialFor } from './trial';
    import type { TrialResult } from './trial';
    import type { Endpoint } from './types';

    interface Props {
        /** The endpoint to call, or `null` while a new one is being filled in. */
        endpoint: Endpoint | null;
    }

    let { endpoint }: Props = $props();

    const NO_BODY_METHODS = ['GET', 'HEAD'];

    // The panel is created again for each endpoint, so the props are only read once.
    let trial = $state(untrack(() => (endpoint ? trialFor(endpoint) : null)));
    let result = $state<TrialResult | null>(null);
    let failure = $state<string | null>(null);
    let sending = $state(false);

    const credentials = $derived(authorizationFor(endpoint?.authentication));
    const curl = $derived(trial ? curlFor(trial, window.location.origin) : '');

    function toggleCredentials(event: Event & { currentTarget: HTMLInputElement }) {
        if (trial) {
            trial.authorization = event.currentTarget.checked ? credentials : null;
        }
    }

    async function submit(event: SubmitEvent) {
        event.preventDefault();

        if (!trial) {
            return;
        }

        sending = true;
        failure = null;

        try {
            result = await send(trial);
        } catch (error) {
            result = null;
            failure = error instanceof Error ? error.message : 'The request could not be sent';
        } finally {
            sending = false;
        }
    }
</script>

<aside class="try-it" aria-label="Try it">
    <div class="pane-heading">
        <span>Try it</span>
    </div>

    {#if trial && endpoint}
        <form onsubmit={submit}>
            <div class="request-line">
                <select aria-label="Method" bind:value={trial.method}>
                    {#each endpoint.method as method}
                        <option value={method}>{method}</option>
                    {/each}
                </select>
                <input type="text" class="code" aria-label="Request path" bind:value={trial.url} />
            </div>

            {#if !NO_BODY_METHODS.includes(trial.method)}
                <label for="trial-body">Body</label>
                <textarea id="trial-body" class="code" rows="4" bind:value={trial.body}></textarea>
            {/if}

            {#if credentials}
                <label class="check">
                    <input type="checkbox" checked={trial.authorization !== null} onchange={toggleCredentials} />
                    <span>Send the registered credentials</span>
                </label>
            {/if}

            <button class="btn-primary" type="submit" disabled={sending}>{sending ? 'Sending' : 'Send'}</button>
        </form>

        <div class="block">
            <span class="block-label">curl</span>
            <pre>{curl}</pre>
        </div>

        <div class="block" aria-live="polite">
            <div class="block-heading">
                <span class="block-label">Response</span>
                {#if result}
                    <span class="status" class:status-error={result.status >= 400}>
                        {result.status} {result.statusText} in {result.elapsedMs} ms
                    </span>
                {/if}
            </div>

            {#if failure}
                <pre class="status-error">{failure}</pre>
            {:else if result}
                <pre>{result.body || '(empty body)'}</pre>
            {:else}
                <p class="hint">Send the request to see what the endpoint answers.</p>
            {/if}
        </div>
    {:else}
        <p class="hint">Register the endpoint, then call it from here and see the real answer.</p>
    {/if}
</aside>
