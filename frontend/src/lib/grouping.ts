import { HTTP_METHODS } from './types';
import type { Endpoint } from './types';

/** How the endpoint list is split into sections. */
export const GROUPINGS = ['none', 'group', 'method'] as const;
export type Grouping = (typeof GROUPINGS)[number];

export const GROUPING_LABELS: Record<Grouping, string> = {
    none: 'None',
    group: 'Group',
    method: 'Method',
};

export const UNGROUPED = 'Ungrouped';

/** A titled part of the endpoint list. The flat list is one section without a title. */
export interface EndpointSection {
    title: string | null;
    endpoints: Endpoint[];
}

const STORAGE_KEY = 'mockiapi.grouping';

export function sectionsOf(endpoints: Endpoint[], grouping: Grouping): EndpointSection[] {
    switch (grouping) {
        case 'none':
            return [{ title: null, endpoints }];
        case 'group':
            return byGroup(endpoints);
        case 'method':
            return byMethod(endpoints);
    }
}

/** The groups in use, in alphabetical order. */
export function groupsOf(endpoints: Endpoint[]): string[] {
    const groups = new Set<string>();

    for (const endpoint of endpoints) {
        if (endpoint.group) {
            groups.add(endpoint.group);
        }
    }

    return [...groups].sort((first, second) => first.localeCompare(second));
}

/** Named groups in alphabetical order, then the endpoints that have none. */
function byGroup(endpoints: Endpoint[]): EndpointSection[] {
    const sections = groupsOf(endpoints).map((group) => ({
        title: group,
        endpoints: endpoints.filter((endpoint) => endpoint.group === group),
    }));
    const ungrouped = endpoints.filter((endpoint) => !endpoint.group);

    if (ungrouped.length > 0) {
        sections.push({ title: UNGROUPED, endpoints: ungrouped });
    }

    return sections;
}

/** One section per method in use. An endpoint that answers several methods is in each of them. */
function byMethod(endpoints: Endpoint[]): EndpointSection[] {
    const inUse = new Set(endpoints.flatMap((endpoint) => endpoint.method.map(normalized)));
    const known = HTTP_METHODS.filter((method) => inUse.has(method));
    const others = [...inUse]
        .filter((method) => !known.some((knownMethod) => knownMethod === method))
        .sort();

    return [...known, ...others].map((method) => ({
        title: method,
        endpoints: endpoints.filter((endpoint) => endpoint.method.map(normalized).includes(method)),
    }));
}

function normalized(method: string): string {
    return method.toUpperCase();
}

/** The grouping chosen on an earlier visit. Storage can be unavailable, so it is optional. */
export function storedGrouping(): Grouping {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);

        return GROUPINGS.find((grouping) => grouping === stored) ?? 'none';
    } catch {
        return 'none';
    }
}

export function storeGrouping(grouping: Grouping): void {
    try {
        localStorage.setItem(STORAGE_KEY, grouping);
    } catch {
        // The choice just does not survive a reload.
    }
}
