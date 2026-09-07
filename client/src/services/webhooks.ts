import type { Webhook, WebhookInput } from 'src/types/webhooks';
import type { ApiResponse } from 'src/types/api_response';
import { api } from './axios';

export async function fetchWebhooks(): Promise<ApiResponse<Webhook[]>> {
    const res = await api.get<ApiResponse<Webhook[]>>('/api/v1/webhooks');
    return res.data;
}

export async function createWebhook(input: WebhookInput): Promise<ApiResponse<Webhook>> {
    const res = await api.post<ApiResponse<Webhook>>('/api/v1/webhooks', input);
    return res.data;
}

export async function updateWebhook(id: string, input: WebhookInput): Promise<ApiResponse<Webhook>> {
    const res = await api.put<ApiResponse<Webhook>>(`/api/v1/webhooks/${id}`, input);
    return res.data;
}

export async function deleteWebhook(id: string): Promise<ApiResponse<string>> {
    const res = await api.delete<ApiResponse<string>>(`/api/v1/webhooks/${id}`);
    return res.data;
}

export async function testWebhook(id: string): Promise<ApiResponse<string>> {
    const res = await api.post<ApiResponse<string>>(`/api/v1/webhooks/${id}/test`);
    return res.data;
}
