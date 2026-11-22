<script lang="ts">
    import { onMount, untrack } from 'svelte';
    import { fetchContent } from './api';
    import {
        canKeepResponse,
        draftOf,
        emptyDraft,
        formatJson,
        GRAPHQL_PATH,
        GRPC_PATH,
        toFormData,
    } from './registration';
    import type { ResponseSource } from './registration';
    import { ENDPOINT_KINDS, HTTP_METHODS } from './types';
    import type { Endpoint, EndpointKind } from './types';

    interface Props {
        /** The endpoint being changed, or `null` to register a new one. */
        endpoint: Endpoint | null;
        /** The groups other endpoints already use, offered as suggestions. */
        groups: string[];
        /** Sends the form and tells whether the server accepted it. */
        onsave: (form: FormData) => Promise<boolean>;
        ondelete: (endpoint: Endpoint) => void;
        /** Called with the reason when the draft cannot be sent as it is. */
        oninvalid: (reason: string) => void;
    }

    let { endpoint, groups, onsave, ondelete, oninvalid }: Props = $props();

    // The editor is created again for each endpoint, so the props are only read once.
    const editingId = untrack(() => endpoint?.id ?? null);
    const editing = editingId !== null;
    let kind = $state<EndpointKind>(untrack(() => endpoint?.type ?? 'REST'));
    let draft = $state(untrack(() => (endpoint ? draftOf(endpoint) : emptyDraft())));
    let fileInput = $state<HTMLInputElement>();
    /** The response the endpoint holds on the server, once it has been loaded. */
    let storedText = $state<string | null>(null);

    const RESPONSE_SOURCES: { source: ResponseSource; label: string }[] = [
        { source: 'file', label: 'File' },
        { source: 'text', label: 'Text' },
    ];

    onMount(showStoredResponse);

    /** A gRPC endpoint holds one response per mock, so there is no single one to show. */
    async function showStoredResponse() {
        if (!endpoint || endpoint.type === 'gRPC') {
            return;
        }

        try {
            storedText = await fetchContent(endpoint.id);
            draft.responseText = storedText;
            draft.responseSource = 'text';
        } catch {
            // The editor still works without it: a file can be chosen to replace the response.
        }
    }

    const fixedPath = $derived(kind === 'GraphQL' ? GRAPHQL_PATH : GRPC_PATH);
    const title = $derived(kind === 'REST' ? draft.path || 'New endpoint' : fixedPath);
    const fileIsOptional = $derived(canKeepResponse(kind, editing));

    function selectFile(event: Event & { currentTarget: HTMLInputElement }) {
        draft.file = event.currentTarget.files?.[0] ?? null;
    }

    /** The chosen file lives in the element, not in the draft. */
    function clearFile() {
        draft.file = null;

        if (fileInput) {
            fileInput.value = '';
        }
    }

    function format() {
        try {
            draft.responseText = formatJson(draft.responseText);
        } catch (error) {
            oninvalid(error instanceof Error ? error.message : 'The response could not be formatted');
        }
    }

    async function submit(event: SubmitEvent) {
        event.preventDefault();

        let form: FormData;

        try {
            form = toFormData(kind, draft, editingId, storedText);
        } catch (error) {
            oninvalid(error instanceof Error ? error.message : 'The form could not be sent');
            return;
        }

        if (await onsave(form)) {
            clearFile();
        }
    }
</script>

<section class="editor">
    <header class="editor-heading">
        <h1 title={title}>{title}</h1>
        <span class="kind">{kind}</span>
    </header>

    {#if !editing}
        <div class="tabs" role="group" aria-label="Endpoint type">
            {#each ENDPOINT_KINDS as tab}
                <button
                    type="button"
                    class="tab-btn"
                    class:tab-active={kind === tab}
                    aria-pressed={kind === tab}
                    onclick={() => (kind = tab)}
                >
                    {tab}
                </button>
            {/each}
        </div>
    {/if}

    <form onsubmit={submit}>
        <fieldset>
            <legend>Route</legend>

            {#if kind === 'REST'}
                <div class="form-group wide">
                    <label for="path">Path</label>
                    <input
                        type="text"
                        id="path"
                        class="code"
                        bind:value={draft.path}
                        readonly={editing}
                        placeholder="/api/example"
                    />
                    {#if editing}
                        <small>To answer on another path, register a new endpoint.</small>
                    {/if}
                </div>

                <div class="form-group wide">
                    <span class="group-label">HTTP Methods</span>
                    <div class="checkbox-group">
                        {#each HTTP_METHODS as method}
                            <label class="check">
                                <input type="checkbox" id={method} bind:checked={draft.methods[method]} />
                                <span>{method}</span>
                            </label>
                        {/each}
                    </div>
                    <small>Other methods of this path can have an endpoint of their own.</small>
                </div>
            {:else if kind === 'gRPC'}
                <div class="form-group">
                    <label for="service">Service</label>
                    <input
                        type="text"
                        id="service"
                        class="code"
                        bind:value={draft.grpcService}
                        placeholder="com.example.MyService"
                    />
                </div>
                <div class="form-group">
                    <label for="rpc">RPC Method</label>
                    <input type="text" id="rpc" class="code" bind:value={draft.grpcRpc} placeholder="GetSomething" />
                </div>
                {#if editing}
                    <small class="wide">
                        Every gRPC mock shares {GRPC_PATH}. Fill in a service and a method to add one or replace it.
                    </small>
                {/if}
            {:else}
                <small class="wide">A GraphQL endpoint always answers POST {GRAPHQL_PATH}.</small>
            {/if}

            <div class="form-group wide">
                <label for="group">Group</label>
                <input type="text" id="group" list="known-groups" bind:value={draft.group} placeholder="users" />
                <datalist id="known-groups">
                    {#each groups as group}
                        <option value={group}></option>
                    {/each}
                </datalist>
                <small>Optional. The list of endpoints can be sorted by it.</small>
            </div>

            <div class="form-group wide">
                <div class="response-heading">
                    {#if draft.responseSource === 'file'}
                        <label for="response-file">Response (JSON File)</label>
                    {:else}
                        <label for="response-text">Response (JSON Text)</label>
                    {/if}

                    <div class="segmented" role="group" aria-label="Response source">
                        {#each RESPONSE_SOURCES as { source, label }}
                            <button
                                type="button"
                                class:active={draft.responseSource === source}
                                aria-pressed={draft.responseSource === source}
                                onclick={() => (draft.responseSource = source)}
                            >
                                {label}
                            </button>
                        {/each}
                    </div>
                </div>

                <!-- Hidden, not removed: the chosen file lives in the element. -->
                <input
                    type="file"
                    id="response-file"
                    accept="application/json"
                    hidden={draft.responseSource !== 'file'}
                    bind:this={fileInput}
                    onchange={selectFile}
                />

                {#if draft.responseSource === 'text'}
                    <textarea
                        id="response-text"
                        class="code"
                        rows="10"
                        spellcheck="false"
                        bind:value={draft.responseText}
                        placeholder={'{ "message": "Hello, world!" }'}
                    ></textarea>
                    <div class="response-footer">
                        <small>
                            {#if storedText !== null}
                                This is what the endpoint answers now. Change it and save.
                            {:else if fileIsOptional}
                                Leave it empty to keep the current response.
                            {:else}
                                Type or paste the response.
                            {/if}
                        </small>
                        <button class="btn-secondary" type="button" onclick={format}>Format</button>
                    </div>
                {:else if fileIsOptional}
                    <small>Leave it empty to keep the current response.</small>
                {/if}
            </div>
        </fieldset>

        <fieldset>
            <legend>Behaviour</legend>

            <div class="form-group">
                <label for="status">Status Code</label>
                <input type="number" id="status" bind:value={draft.statusCode} placeholder="200" />
            </div>

            <div class="form-group">
                <label for="delay">Delay (ms)</label>
                <input type="number" id="delay" bind:value={draft.delay} placeholder="0" />
            </div>

            <div class="form-group">
                <label for="limit">Rate Limit</label>
                <input type="text" id="limit" bind:value={draft.rateLimit} placeholder="10/1000" />
                <small>Requests / window in ms</small>
            </div>

            {#if kind === 'REST'}
                <div class="form-group">
                    <span class="group-label">Dynamic Vars</span>
                    <label class="check">
                        <input type="checkbox" id="with-dynamic-vars" bind:checked={draft.dynamicVars} />
                        <span>Replace {'{{name}}'} placeholders in the response</span>
                    </label>
                </div>
            {/if}
        </fieldset>

        <fieldset>
            <legend>Authentication</legend>

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
                <div class="form-group wide">
                    <label for="token">Token Data (JSON)</label>
                    <textarea id="token" class="code" rows="3" bind:value={draft.tokenData}></textarea>
                </div>
            {/if}
        </fieldset>

        <div class="actions">
            <button class="btn-primary" type="submit">{editing ? 'Save' : 'Register'}</button>

            {#if endpoint}
                <button class="btn-danger" type="button" onclick={() => ondelete(endpoint)}>Delete</button>
            {/if}
        </div>
    </form>
</section>
