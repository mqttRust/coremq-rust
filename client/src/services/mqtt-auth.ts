import type { AuthConfig } from 'src/types/mqtt-auth';
import type { ApiResponse } from 'src/types/api_response';
import { api } from './axios';

export async function getAuthConfig(): Promise<ApiResponse<AuthConfig>> {
    const res = await api.get<ApiResponse<AuthConfig>>('/api/v1/mqtt-auth/config');
    return res.data;
}

export async function updateAuthConfig(cfg: AuthConfig): Promise<ApiResponse<AuthConfig>> {
    const res = await api.put<ApiResponse<AuthConfig>>('/api/v1/mqtt-auth/config', cfg);
    return res.data;
}

export async function listCredentials(): Promise<ApiResponse<string[]>> {
    const res = await api.get<ApiResponse<string[]>>('/api/v1/mqtt-auth/credentials');
    return res.data;
}

export async function createCredential(username: string, password: string): Promise<ApiResponse<string>> {
    const res = await api.post<ApiResponse<string>>('/api/v1/mqtt-auth/credentials', { username, password });
    return res.data;
}

export async function deleteCredential(username: string): Promise<ApiResponse<string>> {
    const res = await api.delete<ApiResponse<string>>(`/api/v1/mqtt-auth/credentials/${encodeURIComponent(username)}`);
    return res.data;
}
