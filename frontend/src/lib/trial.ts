import type { AuthData, Endpoint } from './types';

/** A request the Try it panel is about to send to a mock endpoint. */
export interface Trial {
    method: string;
    /** Path and query, relative to the server. */
    url: string;
    body: string;
    /** The `Authorization` header, or `null` to send none. */
    authorization: string | null;
}

export interface TrialResult {
    status: number;
    statusText: string;
    elapsedMs: number;
    body: string;
}

const BODILESS_METHODS = ['GET', 'HEAD'];

const SAMPLE_BODIES: Record<Endpoint['type'], string> = {
    REST: '',
    GraphQL: '{ "query": "query getUser { id name }" }',
    gRPC: '{ "service": "com.example.BookService", "rpc": "GetBook", "request": {} }',
};

/** The request that calls an endpoint as it was registered, credentials included. */
export function trialFor(endpoint: Endpoint): Trial {
    return {
        method: endpoint.method[0] ?? 'GET',
        url: endpoint.path,
        body: SAMPLE_BODIES[endpoint.type],
        authorization: authorizationFor(endpoint.authentication),
    };
}

export function authorizationFor(authentication: AuthData | undefined): string | null {
    if (authentication?.tokenData) {
        return `Bearer ${authentication.tokenData}`;
    }

    if (authentication) {
        return `Basic ${btoa(`${authentication.username ?? ''}:${authentication.password ?? ''}`)}`;
    }

    return null;
}

function sendsBody(trial: Trial): boolean {
    return trial.body.trim() !== '' && !BODILESS_METHODS.includes(trial.method);
}

/** The `curl` command that sends the same request from a terminal. */
export function curlFor(trial: Trial, origin: string): string {
    const parts = ['curl -i'];

    if (trial.method !== 'GET') {
        parts.push(`-X ${trial.method}`);
    }

    if (trial.authorization !== null) {
        parts.push(`-H ${shellQuote(`Authorization: ${trial.authorization}`)}`);
    }

    if (sendsBody(trial)) {
        parts.push(`-d ${shellQuote(trial.body.trim())}`);
    }

    parts.push(shellQuote(origin + trial.url));

    return parts.join(' ');
}

function shellQuote(text: string): string {
    return `'${text.replaceAll("'", "'\\''")}'`;
}

export async function send(trial: Trial): Promise<TrialResult> {
    const headers = new Headers();

    if (trial.authorization !== null) {
        headers.set('Authorization', trial.authorization);
    }

    const started = performance.now();
    const response = await fetch(trial.url, {
        method: trial.method,
        headers,
        body: sendsBody(trial) ? trial.body : undefined,
    });
    const body = await response.text();
    const elapsedMs = Math.round(performance.now() - started);

    return { status: response.status, statusText: response.statusText, elapsedMs, body: readable(body) };
}

/** Indents a JSON body. Anything else is shown as it came. */
function readable(body: string): string {
    try {
        return JSON.stringify(JSON.parse(body), null, 2);
    } catch {
        return body;
    }
}
