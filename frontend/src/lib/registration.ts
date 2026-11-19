import { HTTP_METHODS } from './types';
import type { AuthType, EndpointKind, HttpMethod } from './types';

const GRAPHQL_PATH = '/api/graphql';
const GRPC_PATH = '/grpc';

/** What the registration form holds while it is being filled in. */
export interface RegistrationDraft {
    path: string;
    methods: Record<HttpMethod, boolean>;
    /** A number input holds `null` while it is empty. */
    statusCode: number | null;
    delay: number | null;
    rateLimit: string;
    dynamicVars: boolean;
    authType: AuthType;
    username: string;
    password: string;
    tokenData: string;
    grpcService: string;
    grpcRpc: string;
    file: File | null;
}

/** A draft the user has to fix before it can be sent. */
export class InvalidDraft extends Error {}

export function emptyDraft(): RegistrationDraft {
    return {
        path: '',
        methods: { GET: true, POST: false, PUT: false, DELETE: false },
        statusCode: null,
        delay: null,
        rateLimit: '',
        dynamicVars: false,
        authType: 'none',
        username: '',
        password: '',
        tokenData: '',
        grpcService: '',
        grpcRpc: '',
        file: null,
    };
}

/**
 * Builds the multipart form `POST /register` expects.
 *
 * GraphQL and gRPC endpoints live on a fixed path and only answer POST, so the path and
 * the methods of the draft only count for REST. Optional fields left empty are not sent.
 */
export function toFormData(kind: EndpointKind, draft: RegistrationDraft): FormData {
    if (draft.file === null) {
        throw new InvalidDraft('Please select a JSON file');
    }

    const form = new FormData();
    form.append('path', pathOf(kind, draft));
    form.append('methods', methodsOf(kind, draft).join(','));
    form.append('isGraphQL', String(kind === 'GraphQL'));
    form.append('with_dynamic_vars', String(kind === 'REST' && draft.dynamicVars));

    appendOptional(form, 'status_code', draft.statusCode);
    appendOptional(form, 'delay', draft.delay);
    appendOptional(form, 'rate_limit', draft.rateLimit.trim());
    appendOptional(form, 'authentication', authenticationOf(draft));

    if (kind === 'gRPC') {
        form.append('grpcService', draft.grpcService.trim());
        form.append('grpcRPC', draft.grpcRpc.trim());
    }

    form.append('file', draft.file);

    return form;
}

function pathOf(kind: EndpointKind, draft: RegistrationDraft): string {
    switch (kind) {
        case 'GraphQL':
            return GRAPHQL_PATH;
        case 'gRPC':
            return GRPC_PATH;
        case 'REST':
            return draft.path.trim();
    }
}

function methodsOf(kind: EndpointKind, draft: RegistrationDraft): HttpMethod[] {
    if (kind !== 'REST') {
        return ['POST'];
    }

    const selected = HTTP_METHODS.filter((method) => draft.methods[method]);

    if (selected.length === 0) {
        throw new InvalidDraft('Please select at least one HTTP method');
    }

    return selected;
}

/** The credentials as the JSON text the server parses, or `null` for an open endpoint. */
function authenticationOf(draft: RegistrationDraft): string | null {
    switch (draft.authType) {
        case 'none':
            return null;
        case 'basic':
            return JSON.stringify({ username: draft.username, password: draft.password });
        case 'token':
            return tokenAuthenticationOf(draft.tokenData);
    }
}

function tokenAuthenticationOf(tokenData: string): string {
    try {
        return JSON.stringify(JSON.parse(tokenData));
    } catch {
        throw new InvalidDraft('Invalid token data format.');
    }
}

function appendOptional(form: FormData, name: string, value: string | number | null): void {
    if (value === null || value === '' || Number.isNaN(value)) {
        return;
    }

    form.append(name, String(value));
}
