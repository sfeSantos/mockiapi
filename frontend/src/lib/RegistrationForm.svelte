<script lang="ts">
    import { emptyDraft, toFormData } from './registration';
    import { ENDPOINT_KINDS, HTTP_METHODS } from './types';
    import type { EndpointKind } from './types';

    interface Props {
        /** Sends the form and tells whether the server accepted it. */
        onregister: (form: FormData) => Promise<boolean>;
        /** Called with the reason when the draft cannot be sent as it is. */
        oninvalid: (reason: string) => void;
    }

    let { onregister, oninvalid }: Props = $props();

    let kind = $state<EndpointKind>('REST');
    let draft = $state(emptyDraft());
    let fileInput = $state<HTMLInputElement>();

    function selectFile(event: Event & { currentTarget: HTMLInputElement }) {
        draft.file = event.currentTarget.files?.[0] ?? null;
    }

    /** Clears the fields and stays on the same tab, ready for another endpoint of that kind. */
    function reset() {
        draft = emptyDraft();

        // The chosen file lives in the element, not in the draft.
        if (fileInput) {
            fileInput.value = '';
        }
    }

    async function submit(event: SubmitEvent) {
        event.preventDefault();

        let form: FormData;

        try {
            form = toFormData(kind, draft);
        } catch (error) {
            oninvalid(error instanceof Error ? error.message : 'The form could not be sent');
            return;
        }

        if (await onregister(form)) {
            reset();
        }
    }
</script>

<section class="registration-form">
    <h2>➕ Register New Endpoint</h2>

    <div class="tabs">
        {#each ENDPOINT_KINDS as tab}
            <button
                type="button"
                class="tab-btn"
                class:tab-active={kind === tab}
                onclick={() => (kind = tab)}
            >
                {tab}
            </button>
        {/each}
    </div>

    <form onsubmit={submit}>
        {#if kind === 'REST'}
            <div class="form-group">
                <label for="path">Path</label>
                <input type="text" id="path" bind:value={draft.path} placeholder="/api/example" />
            </div>

            <div class="form-group">
                <span class="group-label">HTTP Methods</span>
                <div class="checkbox-group">
                    {#each HTTP_METHODS as method}
                        <div>
                            <input type="checkbox" id={method} bind:checked={draft.methods[method]} />
                            <label for={method}>{method}</label>
                        </div>
                    {/each}
                </div>
            </div>
        {:else if kind === 'gRPC'}
            <div class="form-group">
                <label for="service">Service</label>
                <input type="text" id="service" bind:value={draft.grpcService} placeholder="com.example.MyService" />
            </div>
            <div class="form-group">
                <label for="rpc">RPC Method</label>
                <input type="text" id="rpc" bind:value={draft.grpcRpc} placeholder="GetSomething" />
            </div>
        {/if}

        <div class="form-group">
            <label for="status">Status Code</label>
            <input type="number" id="status" bind:value={draft.statusCode} />
        </div>

        <div class="form-group">
            <label for="delay">Delay (ms)</label>
            <input type="number" id="delay" bind:value={draft.delay} />
        </div>

        <div class="form-group">
            <label for="limit">Rate Limit</label>
            <input type="text" id="limit" bind:value={draft.rateLimit} placeholder="10/1000 (requests / window in ms)" />
        </div>

        {#if kind === 'REST'}
            <div class="form-group">
                <span class="group-label">Dynamic Vars</span>
                <div class="checkbox-group">
                    <div>
                        <input type="checkbox" id="with-dynamic-vars" bind:checked={draft.dynamicVars} />
                        <label for="with-dynamic-vars">Replace {'{{name}}'} placeholders in the response</label>
                    </div>
                </div>
            </div>
        {/if}

        <div class="form-group">
            <label for="auth">Authentication Type</label>
            <select id="auth" bind:value={draft.authType}>
                <option value="none">No Auth</option>
                <option value="basic">Basic Auth</option>
                <option value="token">Token Auth</option>
            </select>
        </div>

        {#if draft.authType === 'basic'}
            <div class="form-group">
                <label for="username">Username</label>
                <input type="text" id="username" bind:value={draft.username} />
            </div>
            <div class="form-group">
                <label for="pwd">Password</label>
                <input type="password" id="pwd" bind:value={draft.password} />
            </div>
        {:else if draft.authType === 'token'}
            <div class="form-group">
                <label for="token">Token Data (JSON)</label>
                <textarea id="token" rows="4" bind:value={draft.tokenData}></textarea>
            </div>
        {/if}

        <div class="form-group">
            <label for="response-file">Response (JSON File)</label>
            <input
                type="file"
                id="response-file"
                accept="application/json"
                bind:this={fileInput}
                onchange={selectFile}
            />
        </div>

        <button class="btn-primary" type="submit">Register</button>
    </form>
</section>
