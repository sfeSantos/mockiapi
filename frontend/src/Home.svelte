<script lang="ts">
    import { onMount } from 'svelte';
    import { deleteEndpoint, listEndpoints, registerEndpoint } from './lib/api';
    import EndpointList from './lib/EndpointList.svelte';
    import RegistrationForm from './lib/RegistrationForm.svelte';
    import type { Endpoint, Notice, NoticeKind } from './lib/types';

    const NOTICE_LIFETIME_MS = 5000;
    const currentYear = new Date().getFullYear();

    let endpoints = $state<Endpoint[]>([]);
    let loading = $state(false);
    let notices = $state<Notice[]>([]);
    let nextNoticeId = 0;

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

    async function register(form: FormData): Promise<boolean> {
        const registered = await attempt(() => registerEndpoint(form), 'Endpoint registered successfully!');

        if (registered) {
            await refresh();
        }

        return registered;
    }

    async function remove(path: string) {
        if (!confirm(`Are you sure you want to delete the endpoint "${path}"?`)) {
            return;
        }

        if (await attempt(() => deleteEndpoint(path), 'Endpoint deleted successfully!')) {
            await refresh();
        }
    }
</script>

<div class="container">
    {#each notices as notice (notice.id)}
        <div class="notification {notice.kind}" role="status">{notice.message}</div>
    {/each}

    <header>
        <h1>MockiAPI Dashboard</h1>
    </header>

    <main>
        <RegistrationForm onregister={register} oninvalid={(reason) => notify('error', reason)} />
        <EndpointList {endpoints} {loading} ondelete={remove} />
    </main>

    <footer>
        MockiAPI &copy; {currentYear} — REST • GraphQL • gRPC
    </footer>
</div>
