import type { Endpoint } from './types';

/** `GET /list` answers an object keyed by path. */
type EndpointsByPath = Record<string, Omit<Endpoint, 'path'>>;

export async function listEndpoints(): Promise<Endpoint[]> {
    const response = await fetch('/list');

    if (!response.ok) {
        throw new Error('Failed to load endpoints');
    }

    const endpointsByPath: EndpointsByPath = await response.json();

    return Object.entries(endpointsByPath)
        .map(([path, endpoint]) => ({ path, ...endpoint }));
}

/** The response an endpoint was registered with, as the text that was uploaded. */
export async function fetchContent(path: string): Promise<string> {
    const response = await fetch(`/content/${encodeURIComponent(path)}`);

    if (!response.ok) {
        throw new Error('Failed to load the response of the endpoint');
    }

    return response.text();
}

export async function registerEndpoint(form: FormData): Promise<void> {
    const response = await fetch('/register', { method: 'POST', body: form });

    if (!response.ok) {
        throw new Error(await refusalReason(response));
    }
}

export async function deleteEndpoint(path: string): Promise<void> {
    const response = await fetch(`/delete/${encodeURIComponent(path)}`, { method: 'DELETE' });

    if (!response.ok) {
        throw new Error('Failed to delete endpoint');
    }
}

/** The server answers a refused form with the reason as a JSON string. */
async function refusalReason(response: Response): Promise<string> {
    try {
        const reason: unknown = await response.json();

        if (typeof reason === 'string') {
            return reason;
        }
    } catch {
        // Not JSON: fall back to the generic message.
    }

    return 'Failed to register endpoint';
}
