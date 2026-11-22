<script lang="ts">
    import { onMount } from 'svelte';
    import { deleteEndpoint, importMocks, listEndpoints, registerEndpoint } from './lib/api';
    import EndpointEditor from './lib/EndpointEditor.svelte';
    import EndpointNav from './lib/EndpointNav.svelte';
    import TryIt from './lib/TryIt.svelte';
    import { groupsOf } from './lib/grouping';
    import type { Endpoint, Notice, NoticeKind } from './lib/types';

    const NOTICE_LIFETIME_MS = 5000;
    /** More skipped requests than this are counted, not listed, so the notices stay readable. */
    const LISTED_WARNINGS = 3;
    const currentYear = new Date().getFullYear();
    const origin = window.location.origin;

    let endpoints = $state<Endpoint[]>([]);
    /** The id of the endpoint open in the editor. `null` is a new endpoint. */
    let selectedId = $state<number | null>(null);
    /** Goes up after every save, so the editor and the panel start again from what the server holds. */
    let revision = $state(0);
    let loading = $state(false);
    let notices = $state<Notice[]>([]);
    let nextNoticeId = 0;
    let importInput = $state<HTMLInputElement>();

    const selected = $derived(endpoints.find((endpoint) => endpoint.id === selectedId) ?? null);
    const groups = $derived(groupsOf(endpoints));

    onMount(refresh);

    function notify(kind: NoticeKind, message: string) {
        const id = nextNoticeId++;
        notices.unshift({ id, kind, message });

        setTimeout(() => {
            notices = notices.filter((notice) => notice.id !== id);
        }, NOTICE_LIFETIME_MS);
    }

    /** Runs a server call with the loader on and tells whether it went through. */
    async function attempt(action: () => Promise<void>, success?: string): Promise<boolean> {
        loading = true;

        try {
            await action();

            if (success) {
                notify('success', success);
            }

            return true;
        } catch (error) {
            notify('error', error instanceof Error ? error.message : 'Something went wrong');

            return false;
        } finally {
            loading = false;
        }
    }

    async function refresh() {
        await attempt(async () => {
            endpoints = await listEndpoints();
        });
    }

    async function save(form: FormData): Promise<boolean> {
        const updating = selectedId !== null;
        const success = updating ? 'Endpoint saved successfully!' : 'Endpoint registered successfully!';
        const saved = await attempt(() => registerEndpoint(form), success);

        if (saved) {
            await refresh();
            selectedId = savedBy(form)?.id ?? null;
            revision++;
        }

        return saved;
    }

    /**
     * The endpoint a form that was just accepted ended up as.
     *
     * A path holds one endpoint per method, so the path and any method of the form name it.
     */
    function savedBy(form: FormData): Endpoint | undefined {
        const path = String(form.get('path'));
        const methods = String(form.get('methods')).split(',');

        return endpoints.find(
            (endpoint) => endpoint.path === path && endpoint.method.some((method) => methods.includes(method)),
        );
    }

    /** Sends the chosen mocks file and shows what the server did with it. */
    async function importFile(event: Event & { currentTarget: HTMLInputElement }) {
        const input = event.currentTarget;
        const file = input.files?.[0];
        // Cleared right away, so choosing the same file again fires the event again.
        input.value = '';

        if (!file) {
            return;
        }

        const imported = await attempt(async () => {
            const result = await importMocks(file);
            const endpointWord = result.registered === 1 ? 'endpoint' : 'endpoints';

            notify('success', `Imported ${result.registered} ${endpointWord} from ${file.name}`);
            warn(result.warnings);
        });

        if (imported) {
            selectedId = null;
            await refresh();
        }
    }

    function warn(warnings: string[]) {
        for (const warning of warnings.slice(0, LISTED_WARNINGS)) {
            notify('error', warning);
        }

        if (warnings.length > LISTED_WARNINGS) {
            notify('error', `${warnings.length - LISTED_WARNINGS} more requests were skipped`);
        }
    }

    async function remove(endpoint: Endpoint) {
        const name = `${endpoint.method.join(', ')} ${endpoint.path}`;

        if (!confirm(`Are you sure you want to delete the endpoint "${name}"?`)) {
            return;
        }

        if (await attempt(() => deleteEndpoint(endpoint.id), 'Endpoint deleted successfully!')) {
            selectedId = null;
            await refresh();
        }
    }
</script>

<div class="workbench">
    <header class="top-bar">
        <div class="brand">
            <span class="name">MockiAPI Dashboard</span>
            <span class="origin">{origin}</span>
        </div>
        <div class="top-actions">
            <input
                type="file"
                accept=".json,.yaml,.yml,application/json"
                hidden
                bind:this={importInput}
                onchange={importFile}
            />
            <button
                class="btn-secondary"
                type="button"
                title="Register the endpoints of a mocks file (JSON or YAML) or of a Postman collection"
                onclick={() => importInput?.click()}
            >
                Import file
            </button>
            <button class="btn-primary" type="button" onclick={() => (selectedId = null)}>New endpoint</button>
        </div>
    </header>

    <div class="notices" aria-live="polite">
        {#each notices as notice (notice.id)}
            <div class="notification {notice.kind}">{notice.message}</div>
        {/each}
    </div>

    <main>
        <EndpointNav {endpoints} {selectedId} {loading} onselect={(id) => (selectedId = id)} />

        <!-- Each endpoint gets a fresh editor and a fresh panel, so nothing leaks between them. -->
        {#key `${selected?.id ?? ''}#${revision}`}
            <EndpointEditor
                endpoint={selected}
                {groups}
                onsave={save}
                ondelete={remove}
                oninvalid={(reason) => notify('error', reason)}
            />
            <TryIt endpoint={selected} />
        {/key}
    </main>

    <footer>
        MockiAPI &copy; {currentYear} — REST • GraphQL • gRPC
    </footer>
</div>
