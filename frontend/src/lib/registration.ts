import { HTTP_METHODS } from './types';
import type { AuthData, AuthType, Endpoint, EndpointKind, HttpMethod } from './types';

export const GRAPHQL_PATH = '/api/graphql';
export const GRPC_PATH = '/grpc';

/** Where the response comes from: a file on the user's machine, or text typed into the page. */
export type ResponseSource = 'file' | 'text';

/** What the editor holds while an endpoint is being filled in. */
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
    responseSource: ResponseSource;
    file: File | null;
    responseText: string;
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
        responseSource: 'file',
        file: null,
        responseText: '',
    };
}

/** The draft that shows a registered endpoint as it is, ready to be changed. */
export function draftOf(endpoint: Endpoint): RegistrationDraft {
    const allowed = endpoint.method.map((method) => method.toUpperCase());
    const rateLimit = endpoint.rate_limit;

    return {
        ...emptyDraft(),
        ...credentialsOf(endpoint.authentication),
        path: endpoint.path,
        methods: {
            GET: allowed.includes('GET'),
            POST: allowed.includes('POST'),
            PUT: allowed.includes('PUT'),
            DELETE: allowed.includes('DELETE'),
        },
        statusCode: endpoint.status_code,
        delay: endpoint.delay,
        rateLimit: rateLimit ? `${rateLimit.requests}/${rateLimit.window_ms}` : '',
        dynamicVars: endpoint.with_dynamic_vars === true,
    };
}

function credentialsOf(
    authentication: AuthData | undefined,
): Pick<RegistrationDraft, 'authType' | 'username' | 'password' | 'tokenData'> {
    if (authentication?.tokenData) {
        const tokenData = JSON.stringify({ tokenData: authentication.tokenData });

        return { authType: 'token', username: '', password: '', tokenData };
    }

    if (authentication) {
        return {
            authType: 'basic',
            username: authentication.username ?? '',
            password: authentication.password ?? '',
            tokenData: '',
        };
    }

    return { authType: 'none', username: '', password: '', tokenData: '' };
}

/**
 * Whether the server accepts this registration without a file.
 *
 * It does when a REST or GraphQL endpoint is being changed: the response it already has is
 * kept. A new endpoint needs one, and so does every gRPC mock.
 */
export function canKeepResponse(kind: EndpointKind, editing: boolean): boolean {
    return editing && kind !== 'gRPC';
}

/**
 * Builds the multipart form `POST /register` expects.
 *
 * GraphQL and gRPC endpoints live on a fixed path and only answer POST, so the path and
 * the methods of the draft only count for REST. Optional fields left empty are not sent.
 *
 * `storedText` is the response the endpoint holds on the server, when it was loaded.
 * Text that still equals it is not sent again.
 */
export function toFormData(
    kind: EndpointKind,
    draft: RegistrationDraft,
    editing: boolean,
    storedText: string | null = null,
): FormData {
    const response = responseOf(kind, draft, storedText);

    if (response === null && !canKeepResponse(kind, editing)) {
        throw new InvalidDraft(MISSING_RESPONSE[draft.responseSource]);
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

    if (response !== null) {
        form.append('file', response);
    }

    return form;
}

const MISSING_RESPONSE: Record<ResponseSource, string> = {
    file: 'Please select a JSON file',
    text: 'Please type or paste the response',
};

/** The response to upload, or `null` when the draft brings nothing new. */
function responseOf(kind: EndpointKind, draft: RegistrationDraft, storedText: string | null): File | null {
    if (draft.responseSource === 'file') {
        return draft.file;
    }

    const text = draft.responseText;

    if (text.trim() === '' || text === storedText) {
        return null;
    }

    // A REST response may be a template that only becomes JSON once its placeholders are filled.
    if (kind !== 'REST' && !isJson(text)) {
        throw new InvalidDraft('The response must be valid JSON');
    }

    return new File([text], 'response.json', { type: 'application/json' });
}

function isJson(text: string): boolean {
    try {
        JSON.parse(text);

        return true;
    } catch {
        return false;
    }
}

/** Indents JSON text. A template with bare placeholders is not JSON and cannot be formatted. */
export function formatJson(text: string): string {
    try {
        return JSON.stringify(JSON.parse(text), null, 2);
    } catch {
        throw new InvalidDraft('The response is not valid JSON, so it cannot be formatted');
    }
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
