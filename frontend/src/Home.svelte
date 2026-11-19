<script lang="ts">
    import { onMount } from 'svelte';
    import { deleteEndpoint, listEndpoints, registerEndpoint } from './lib/api';
    import EndpointEditor from './lib/EndpointEditor.svelte';
    import EndpointNav from './lib/EndpointNav.svelte';
    import TryIt from './lib/TryIt.svelte';
    import type { Endpoint, Notice, NoticeKind } from './lib/types';

    const NOTICE_LIFETIME_MS = 5000;
    const currentYear = new Date().getFullYear();
    const origin = window.location.origin;

    let endpoints = $state<Endpoint[]>([]);
    /** The registered path of the endpoint open in the editor. `null` is a new endpoint. */
    let selectedPath = $state<string | null>(null);
    /** Goes up after every save, so the editor and the panel start again from what the server holds. */
    let revision = $state(0);
    let loading = $state(false);
    let notices = $state<Notice[]>([]);
    let nextNoticeId = 0;

    const selected = $derived(endpoints.find((endpoint) => endpoint.path === selectedPath) ?? null);

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
        const updating = selectedPath !== null;
        const success = updating ? 'Endpoint saved successfully!' : 'Endpoint registered successfully!';
        const saved = await attempt(() => registerEndpoint(form), success);

        if (saved) {
            await refresh();
            selectedPath = String(form.get('path'));
            revision++;
        }

        return saved;
    }

    async function remove(path: string) {
        if (!confirm(`Are you sure you want to delete the endpoint "${path}"?`)) {
            return;
        }

        if (await attempt(() => deleteEndpoint(path), 'Endpoint deleted successfully!')) {
            selectedPath = null;
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
        <button class="btn-primary" type="button" onclick={() => (selectedPath = null)}>New endpoint</button>
    </header>

    <div class="notices" aria-live="polite">
        {#each notices as notice (notice.id)}
            <div class="notification {notice.kind}">{notice.message}</div>
        {/each}
    </div>

    <main>
        <EndpointNav {endpoints} {selectedPath} {loading} onselect={(path) => (selectedPath = path)} />

        <!-- Each endpoint gets a fresh editor and a fresh panel, so nothing leaks between them. -->
        {#key `${selected?.path ?? ''}#${revision}`}
            <EndpointEditor
                endpoint={selected}
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
