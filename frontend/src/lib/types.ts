export const HTTP_METHODS = ['GET', 'POST', 'PUT', 'DELETE'] as const;
export type HttpMethod = (typeof HTTP_METHODS)[number];

export const ENDPOINT_KINDS = ['REST', 'GraphQL', 'gRPC'] as const;
export type EndpointKind = (typeof ENDPOINT_KINDS)[number];

export type AuthType = 'none' | 'basic' | 'token';

export interface RateLimit {
    requests: number;
    window_ms: number;
}

/** Basic auth fills the username and password, a bearer token fills `tokenData`. */
export interface AuthData {
    username: string | null;
    password: string | null;
    tokenData: string | null;
}

/** An endpoint as `GET /list` describes it, with the path it is registered on. */
export interface Endpoint {
    path: string;
    type: EndpointKind;
    method: string[];
    file: string;
    status_code: number | null;
    delay: number | null;
    rate_limit: RateLimit | null;
    with_dynamic_vars: boolean | null;
    /** Present only when the endpoint asks for credentials. */
    authentication?: AuthData;
}

export type NoticeKind = 'success' | 'error';

export interface Notice {
    id: number;
    kind: NoticeKind;
    message: string;
}
