export type AuthConfig = {
    allow_anonymous: boolean;
    builtin_enabled: boolean;
    http_enabled: boolean;
    http_url: string;
    jwt_enabled: boolean;
    jwt_secret: string;
};

export const DEFAULT_AUTH_CONFIG: AuthConfig = {
    allow_anonymous: true,
    builtin_enabled: false,
    http_enabled: false,
    http_url: '',
    jwt_enabled: false,
    jwt_secret: '',
};
