import type { Listener, CreateListenerInput } from 'src/types/listeners';
import { api } from './axios';

export async function fetchListeners(): Promise<Listener[]> {
    const res = await api.get<Listener[]>('/api/v1/listeners');
    return res.data;
}

export async function createListener(input: CreateListenerInput) {
    const res = await api.post('/api/v1/listeners', input);
    return res.data;
}

export async function updateListener(port: number, input: CreateListenerInput) {
    const res = await api.put(`/api/v1/listeners/${port}`, input);
    return res.data;
}

export async function stopListener(port: number): Promise<void> {
    await api.delete(`/api/v1/listeners/${port}`);
}
