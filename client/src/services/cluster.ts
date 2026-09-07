import type { ApiResponse } from 'src/types/api_response';
import type {
    ClusterNode,
    ClusterRoute,
    ClusterSession,
    ClusterStatus,
    CreateFederationInput,
    FederationLink,
} from 'src/types/cluster';
import { api } from './axios';

export async function getClusterStatus(): Promise<ApiResponse<ClusterStatus>> {
    const res = await api.get<ApiResponse<ClusterStatus>>('/api/v1/cluster');
    return res.data;
}

export async function listNodes(): Promise<ApiResponse<ClusterNode[]>> {
    const res = await api.get<ApiResponse<ClusterNode[]>>('/api/v1/cluster/nodes');
    return res.data;
}

export async function listRoutes(): Promise<ApiResponse<ClusterRoute[]>> {
    const res = await api.get<ApiResponse<ClusterRoute[]>>('/api/v1/cluster/routes');
    return res.data;
}

export async function listClusterSessions(): Promise<ApiResponse<ClusterSession[]>> {
    const res = await api.get<ApiResponse<ClusterSession[]>>('/api/v1/cluster/sessions');
    return res.data;
}

/** Dial a peer by `host:port`. Discovery normally does this on its own. */
export async function joinNode(address: string): Promise<ApiResponse<string>> {
    const res = await api.post<ApiResponse<string>>('/api/v1/cluster/nodes', { address });
    return res.data;
}

/** Force-remove a node the failure detector has already given up on. */
export async function evictNode(id: string): Promise<ApiResponse<string>> {
    const res = await api.delete<ApiResponse<string>>(
        `/api/v1/cluster/nodes/${encodeURIComponent(id)}`,
    );
    return res.data;
}

export async function listFederation(): Promise<ApiResponse<FederationLink[]>> {
    const res = await api.get<ApiResponse<FederationLink[]>>('/api/v1/cluster/federation');
    return res.data;
}

export async function createFederation(
    input: CreateFederationInput,
): Promise<ApiResponse<string>> {
    const res = await api.post<ApiResponse<string>>('/api/v1/cluster/federation', input);
    return res.data;
}

export async function deleteFederation(name: string): Promise<ApiResponse<string>> {
    const res = await api.delete<ApiResponse<string>>(
        `/api/v1/cluster/federation/${encodeURIComponent(name)}`,
    );
    return res.data;
}
