import { DataProvider } from '@refinedev/core';
import { apiClient } from '../utils/api';

export const dataProvider: DataProvider = {
  getList: async ({ resource, meta }) => {
    const url = `/${resource}`;
    const { data } = await apiClient.get(url, { params: { ...meta } });

    return {
      data: data[resource] || [],
      total: data[resource]?.length || 0,
    };
  },

  getOne: async ({ resource, id, meta }) => {
    const url = `/${resource}/${id}`;
    const { data } = await apiClient.get(url, { params: { ...meta } });

    return { data };
  },

  create: async ({ resource, variables }) => {
    const url = `/${resource}`;
    const { data } = await apiClient.post(url, variables);

    return { data };
  },

  update: async ({ resource, id, variables }) => {
    const url = `/${resource}/${id}`;
    const { data } = await apiClient.put(url, variables);

    return { data };
  },

  deleteOne: async ({ resource, id }) => {
    const url = `/${resource}/${id}`;
    const { data } = await apiClient.delete(url);

    return { data };
  },

  getApiUrl: () => import.meta.env.VITE_API_URL || '/api',

  custom: async ({ url, method, payload, headers, query, meta }) => {
    const { data } = await apiClient.request({
      url,
      method,
      data: payload,
      headers,
      params: query || meta?.query,
    });

    return { data };
  },
};
