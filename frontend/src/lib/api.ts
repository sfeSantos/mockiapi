import type { Endpoint, ImportResult } from './types';

export async function listEndpoints(): Promise<Endpoint[]> {
    const response = await fetch('/list');

    if (!response.ok) {
        throw new Error('Failed to load endpoints');
    }

    return response.json();
}

/** The response an endpoint was registered with, as the text that was uploaded. */
export async function fetchContent(id: number): Promise<string> {
    const response = await fetch(`/content/${id}`);

    if (!response.ok) {
        throw new Error('Failed to load the response of the endpoint');
    }

    return response.text();
}

export async function registerEndpoint(form: FormData): Promise<void> {
    const response = await fetch('/register', { method: 'POST', body: form });

    if (!response.ok) {
        throw new Error(await refusalReason(response, 'Failed to register endpoint'));
    }
}

/**
 * Registers every endpoint of a mocks file: mockiapi's own format in JSON or YAML, or a
 * Postman collection. The server takes the whole file or none of it.
 */
export async function importMocks(file: File): Promise<ImportResult> {
    const form = new FormData();
    form.append('file', file);

    const response = await fetch('/import', { method: 'POST', body: form });

    if (!response.ok) {
        throw new Error(await refusalReason(response, 'Failed to import the file'));
    }

    return response.json();
}

export async function deleteEndpoint(id: number): Promise<void> {
    const response = await fetch(`/delete/${id}`, { method: 'DELETE' });

    if (!response.ok) {
        throw new Error('Failed to delete endpoint');
    }
}

/** The server answers a refused form with the reason as a JSON string. */
async function refusalReason(response: Response, fallback: string): Promise<string> {
    try {
        const reason: unknown = await response.json();

        if (typeof reason === 'string') {
            return reason;
        }
    } catch {
        // Not JSON: fall back to the generic message.
    }

    return fallback;
}
