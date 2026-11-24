import { ENDPOINT_KINDS } from './types';
import type { Endpoint, EndpointKind } from './types';
import { groupsOf } from './grouping';

/** The numbers shown above the workbench, so the state of the server reads at a glance. */
export interface Summary {
    endpoints: number;
    groups: number;
    /** Endpoints that ask for credentials. */
    protected: number;
    /** Endpoints that answer slowly or under a rate limit. */
    shaped: number;
    /** How many endpoints of each kind, for the kinds in use. */
    kinds: { kind: EndpointKind; count: number }[];
}

export function summaryOf(endpoints: Endpoint[]): Summary {
    return {
        endpoints: endpoints.length,
        groups: groupsOf(endpoints).length,
        protected: endpoints.filter((endpoint) => endpoint.authentication !== undefined).length,
        shaped: endpoints.filter(isShaped).length,
        kinds: ENDPOINT_KINDS.map((kind) => ({ kind, count: endpoints.filter((endpoint) => endpoint.type === kind).length }))
            .filter(({ count }) => count > 0),
    };
}

function isShaped(endpoint: Endpoint): boolean {
    return (endpoint.delay ?? 0) > 0 || endpoint.rate_limit !== null;
}

/** The endpoints whose path or group contains the text, whatever its case. */
export function matching(endpoints: Endpoint[], text: string): Endpoint[] {
    const wanted = text.trim().toLowerCase();

    if (wanted === '') {
        return endpoints;
    }

    return endpoints.filter(
        (endpoint) =>
            endpoint.path.toLowerCase().includes(wanted) || (endpoint.group ?? '').toLowerCase().includes(wanted),
    );
}

/** The other endpoints registered on the same path, each answering its own methods. */
export function siblingsOf(endpoints: Endpoint[], path: string, id: number | null): Endpoint[] {
    const wanted = path.trim();

    if (wanted === '') {
        return [];
    }

    return endpoints.filter((endpoint) => endpoint.path === wanted && endpoint.id !== id);
}

/** What an endpoint answers with, short enough for a list: its status code. */
export function statusOf(endpoint: Endpoint): number {
    return endpoint.status_code ?? 200;
}
