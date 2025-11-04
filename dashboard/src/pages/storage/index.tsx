import { useCustom } from '@refinedev/core';
import { Card, Table, Button, Spin, Row, Col, Statistic, Select } from 'antd';
import { DownloadOutlined, DatabaseOutlined, FileOutlined, ClockCircleOutlined } from '@ant-design/icons';
import { PieChart, Pie, Cell, ResponsiveContainer, Legend, Tooltip } from 'recharts';
import { useState } from 'react';

const COLORS = ['#0088FE', '#00C49F', '#FFBB28', '#FF8042', '#8884D8', '#82CA9D', '#FFC658', '#FF6B9D'];

export const StoragePage = () => {
  const { query: statsQuery } = useCustom({
    url: '/storage/stats',
    method: 'get',
  });

  const { data: statsData, isLoading: statsLoading } = statsQuery;
  const buckets = statsData?.data?.buckets || [];
  const [selectedBucketName, setSelectedBucketName] = useState<string>('');

  const firstBucket = buckets[0]?.name;
  const currentBucket = buckets.find((b: any) => b.name === (selectedBucketName || firstBucket)) || buckets[0];

  const bucketToQuery = selectedBucketName || firstBucket;

  const { query: objectsQuery } = useCustom({
    url: `/storage/${bucketToQuery}/objects`,
    method: 'get',
    meta: {
      query: {
        limit: 20,
      },
    },
    queryOptions: {
      enabled: !!bucketToQuery,
    },
  });

  const { data: objectsData, isLoading: objectsLoading } = objectsQuery;

  const objects = objectsData?.data?.objects || [];

  // Prepare data for pie chart
  const pieChartData = currentBucket?.contentTypeBreakdown?.map((item: any) => ({
    name: item.contentType,
    value: item.count,
    size: item.totalSizeBytes,
  })) || [];

  const columns = [
    { title: 'Object Key', dataIndex: 'key', key: 'key' },
    {
      title: 'Size',
      dataIndex: 'size',
      key: 'size',
      render: (size: number) => `${(size / 1024 / 1024).toFixed(2)} MB`,
    },
    { title: 'Last Modified', dataIndex: 'lastModified', key: 'lastModified' },
    {
      title: 'Actions',
      key: 'actions',
      render: (_: any, record: any) => (
        <Button
          icon={<DownloadOutlined />}
          onClick={() => window.open(record.presignedUrl, '_blank')}
        >
          Download
        </Button>
      ),
    },
  ];

  if (statsLoading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <Spin size="large" tip="Loading storage stats..." />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold">Storage Browser</h1>
        {buckets.length > 1 && (
          <Select
            style={{ width: 300 }}
            placeholder="Select bucket"
            value={selectedBucketName || firstBucket}
            onChange={setSelectedBucketName}
          >
            {buckets.map((bucket: any) => (
              <Select.Option key={bucket.name} value={bucket.name}>
                {bucket.name}
              </Select.Option>
            ))}
          </Select>
        )}
      </div>

      {currentBucket && (
        <>
          <Row gutter={[16, 16]}>
            <Col xs={24} sm={12} lg={6}>
              <Card>
                <Statistic
                  title="Total Objects"
                  value={currentBucket.objectCount}
                  prefix={<FileOutlined />}
                  valueStyle={{ color: '#1890ff' }}
                />
              </Card>
            </Col>
            <Col xs={24} sm={12} lg={6}>
              <Card>
                <Statistic
                  title="Total Size"
                  value={(currentBucket.totalSizeBytes / 1024 / 1024 / 1024).toFixed(2)}
                  suffix="GB"
                  prefix={<DatabaseOutlined />}
                  valueStyle={{ color: '#52c41a' }}
                />
              </Card>
            </Col>
            <Col xs={24} sm={12} lg={6}>
              <Card>
                <Statistic
                  title="Oldest Object"
                  value={currentBucket.oldestObject ? new Date(currentBucket.oldestObject).toLocaleDateString() : 'N/A'}
                  prefix={<ClockCircleOutlined />}
                  valueStyle={{ fontSize: '16px' }}
                />
              </Card>
            </Col>
            <Col xs={24} sm={12} lg={6}>
              <Card>
                <Statistic
                  title="Newest Object"
                  value={currentBucket.newestObject ? new Date(currentBucket.newestObject).toLocaleDateString() : 'N/A'}
                  prefix={<ClockCircleOutlined />}
                  valueStyle={{ fontSize: '16px' }}
                />
              </Card>
            </Col>
          </Row>

          {pieChartData.length > 0 && (
            <Card title="Storage Breakdown by Content Type">
              <Row gutter={[16, 16]}>
                <Col xs={24} lg={12}>
                  <ResponsiveContainer width="100%" height={300}>
                    <PieChart>
                      <Pie
                        data={pieChartData}
                        cx="50%"
                        cy="50%"
                        labelLine={false}
                        label={({ name, percent }: any) => `${name.split('/').pop()} (${(percent * 100).toFixed(0)}%)`}
                        outerRadius={80}
                        fill="#8884d8"
                        dataKey="value"
                      >
                        {pieChartData.map((_: any, index: number) => (
                          <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                        ))}
                      </Pie>
                      <Tooltip
                        formatter={(value: any, _name: any, props: any) => [
                          `${value} files (${(props.payload.size / 1024 / 1024).toFixed(2)} MB)`,
                          props.payload.name,
                        ]}
                      />
                      <Legend />
                    </PieChart>
                  </ResponsiveContainer>
                </Col>
                <Col xs={24} lg={12}>
                  <Table
                    dataSource={currentBucket.contentTypeBreakdown}
                    columns={[
                      { title: 'Content Type', dataIndex: 'contentType', key: 'contentType' },
                      { title: 'Count', dataIndex: 'count', key: 'count' },
                      {
                        title: 'Total Size',
                        dataIndex: 'totalSizeBytes',
                        key: 'totalSizeBytes',
                        render: (bytes: number) => `${(bytes / 1024 / 1024).toFixed(2)} MB`,
                      },
                    ]}
                    pagination={false}
                    size="small"
                    rowKey="contentType"
                  />
                </Col>
              </Row>
            </Card>
          )}
        </>
      )}

      <Card title={`Recent Objects (${objects.length})`}>
        <Table
          dataSource={objects}
          columns={columns}
          loading={objectsLoading}
          rowKey="key"
          pagination={{ pageSize: 20 }}
        />
      </Card>
    </div>
  );
};
