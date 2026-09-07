import { ApiResponse } from 'src/types/api_response';
import { api } from './axios';

export type GenerateCertInput = {
    common_name: string;
    sans: string[];
};

export type GeneratedCert = {
    cert: string;
    key: string;
    names: string[];
};

export async function generateCert(input: GenerateCertInput): Promise<ApiResponse<GeneratedCert>> {
    const res = await api.post<ApiResponse<GeneratedCert>>('/api/v1/tls/generate', input);
    return res.data;
}
