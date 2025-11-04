import { useState } from 'react';
import { useCustom, useNavigation } from '@refinedev/core';
import {
  Card,
  Table,
  Tag,
  Badge,
  Input,
  Button,
  Space,
  Select,
  Typography,
  Row,
  Col,
  Statistic,
  Descriptions,
} from 'antd';
import {
  ReloadOutlined,
  SearchOutlined,
  InboxOutlined,
  SendOutlined,
  WarningOutlined,
  ClockCircleOutlined,
  MessageOutlined,
} from '@ant-design/icons';
import { useParams } from 'react-router-dom';
import type { ColumnsType } from 'antd/es/table';

const { Title, Text, Paragraph } = Typography;
const { Search } = Input;

// TypeScript interfaces
interface Queue {
  name: string;
  type: 'inbound' | 'outbound' | 'dlq';
  messageCount: number;
  messagesInFlight: number;
  oldestMessageAge: number;
  url?: string;
  createdAt?: string;
}

interface QueueMessage {
  messageId: string;
  receiptHandle: string;
  body: string;
  attributes: {
    SentTimestamp?: string;
    ApproximateReceiveCount?: string;
    ApproximateFirstReceiveTimestamp?: string;
  };
  messageAttributes?: Record<string, any>;
  md5OfBody?: string;
}

interface QueuesResponse {
  queues: Queue[];
  total: number;
}

interface QueueMessagesResponse {
  messages: QueueMessage[];
  queueInfo: Queue;
}

// Queue type configuration
const queueTypeConfig = {
  inbound: {
    color: 'blue',
    icon: <InboxOutlined />,
    label: 'Inbound',
  },
  outbound: {
    color: 'green',
    icon: <SendOutlined />,
    label: 'Outbound',
  },
  dlq: {
    color: 'red',
    icon: <WarningOutlined />,
    label: 'Dead Letter',
  },
};

// Helper function to format age
const formatAge = (seconds: number): string => {
  if (seconds === 0) return 'N/A';
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
};

// QueuesPage Component (List View)
export const QueuesPage = () => {
  const [searchTerm, setSearchTerm] = useState('');
  const [filterType, setFilterType] = useState<string>('all');
  const { show } = useNavigation();

  const { query } = useCustom<QueuesResponse>({
    url: '/queues',
    method: 'get',
  });

  const { data, isLoading, refetch } = query;

  const queues = data?.data?.queues || [];

  // Filter queues based on search and type
  const filteredQueues = queues.filter((queue) => {
    const matchesSearch = queue.name
      .toLowerCase()
      .includes(searchTerm.toLowerCase());
    const matchesType = filterType === 'all' || queue.type === filterType;
    return matchesSearch && matchesType;
  });

  // Calculate statistics
  const stats = {
    total: queues.length,
    totalMessages: queues.reduce((sum, q) => sum + q.messageCount, 0),
    inFlight: queues.reduce((sum, q) => sum + q.messagesInFlight, 0),
    dlqCount: queues.filter((q) => q.type === 'dlq').length,
  };

  const columns: ColumnsType<Queue> = [
    {
      title: 'Queue Name',
      dataIndex: 'name',
      key: 'name',
      sorter: (a, b) => a.name.localeCompare(b.name),
      render: (name: string, record: Queue) => (
        <Space>
          <Text strong>{name}</Text>
          {record.type === 'dlq' && (
            <Badge status="error" text="DLQ" />
          )}
        </Space>
      ),
    },
    {
      title: 'Type',
      dataIndex: 'type',
      key: 'type',
      width: 120,
      filters: [
        { text: 'Inbound', value: 'inbound' },
        { text: 'Outbound', value: 'outbound' },
        { text: 'Dead Letter', value: 'dlq' },
      ],
      onFilter: (value, record) => record.type === value,
      render: (type: Queue['type']) => {
        const config = queueTypeConfig[type];
        return (
          <Tag color={config.color} icon={config.icon}>
            {config.label}
          </Tag>
        );
      },
    },
    {
      title: 'Messages',
      dataIndex: 'messageCount',
      key: 'messageCount',
      width: 120,
      sorter: (a, b) => a.messageCount - b.messageCount,
      render: (count: number) => (
        <Badge
          count={count}
          showZero
          overflowCount={9999}
          style={{ backgroundColor: count > 0 ? '#1890ff' : '#d9d9d9' }}
        />
      ),
    },
    {
      title: 'In Flight',
      dataIndex: 'messagesInFlight',
      key: 'messagesInFlight',
      width: 120,
      sorter: (a, b) => a.messagesInFlight - b.messagesInFlight,
      render: (count: number) => (
        <Badge
          count={count}
          showZero
          overflowCount={9999}
          style={{ backgroundColor: count > 0 ? '#52c41a' : '#d9d9d9' }}
        />
      ),
    },
    {
      title: 'Oldest Message',
      dataIndex: 'oldestMessageAge',
      key: 'oldestMessageAge',
      width: 140,
      sorter: (a, b) => a.oldestMessageAge - b.oldestMessageAge,
      render: (age: number) => (
        <Space>
          <ClockCircleOutlined />
          <Text type={age > 3600 ? 'danger' : undefined}>
            {formatAge(age)}
          </Text>
        </Space>
      ),
    },
  ];

  return (
    <div className="p-6">
      <div className="mb-6">
        <Title level={2}>
          <InboxOutlined className="mr-2" />
          Queue Management
        </Title>
        <Text type="secondary">
          Monitor and manage SQS queues for email processing
        </Text>
      </div>

      {/* Statistics Cards */}
      <Row gutter={16} className="mb-6">
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Total Queues"
              value={stats.total}
              prefix={<InboxOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Total Messages"
              value={stats.totalMessages}
              prefix={<MessageOutlined />}
              valueStyle={{ color: '#1890ff' }}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="In Flight"
              value={stats.inFlight}
              prefix={<SendOutlined />}
              valueStyle={{ color: '#52c41a' }}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Dead Letter Queues"
              value={stats.dlqCount}
              prefix={<WarningOutlined />}
              valueStyle={{ color: stats.dlqCount > 0 ? '#ff4d4f' : undefined }}
            />
          </Card>
        </Col>
      </Row>

      {/* Filters and Actions */}
      <Card className="mb-4">
        <Row gutter={16} align="middle">
          <Col xs={24} sm={12} md={8} lg={10}>
            <Search
              placeholder="Search queues by name..."
              allowClear
              prefix={<SearchOutlined />}
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              size="large"
            />
          </Col>
          <Col xs={24} sm={12} md={6} lg={6}>
            <Select
              placeholder="Filter by type"
              value={filterType}
              onChange={setFilterType}
              style={{ width: '100%' }}
              size="large"
              options={[
                { label: 'All Types', value: 'all' },
                { label: 'Inbound', value: 'inbound' },
                { label: 'Outbound', value: 'outbound' },
                { label: 'Dead Letter', value: 'dlq' },
              ]}
            />
          </Col>
          <Col xs={24} sm={24} md={10} lg={8} className="text-right">
            <Space>
              <Button
                icon={<ReloadOutlined />}
                onClick={() => refetch()}
                loading={isLoading}
                size="large"
              >
                Refresh
              </Button>
            </Space>
          </Col>
        </Row>
      </Card>

      {/* Queues Table */}
      <Card>
        <Table
          dataSource={filteredQueues}
          columns={columns}
          rowKey="name"
          loading={isLoading}
          pagination={{
            pageSize: 10,
            showSizeChanger: true,
            showTotal: (total) => `Total ${total} queues`,
          }}
          onRow={(record) => ({
            onClick: () => show('queues', record.name),
            style: { cursor: 'pointer' },
          })}
          locale={{
            emptyText: searchTerm || filterType !== 'all'
              ? 'No queues match your filters'
              : 'No queues available',
          }}
        />
      </Card>
    </div>
  );
};

// QueueDetailPage Component (Detail View)
export const QueueDetailPage = () => {
  const { name } = useParams<{ name: string }>();
  const [expandedRowKeys, setExpandedRowKeys] = useState<string[]>([]);

  const { query } = useCustom<QueueMessagesResponse>({
    url: `/queues/${name}/messages`,
    method: 'get',
    queryOptions: {
      enabled: !!name && name !== ':name',
    },
  });

  const { data, isLoading, refetch } = query;

  const messages = data?.data?.messages || [];
  const queueInfo = data?.data?.queueInfo;

  // Parse message body safely
  const parseMessageBody = (body: string): any => {
    try {
      return JSON.parse(body);
    } catch {
      return body;
    }
  };

  // Format JSON with syntax highlighting
  const formatJson = (obj: any): string => {
    return JSON.stringify(obj, null, 2);
  };

  // Format timestamp
  const formatTimestamp = (timestamp?: string): string => {
    if (!timestamp) return 'N/A';
    const date = new Date(parseInt(timestamp));
    return date.toLocaleString();
  };

  const columns: ColumnsType<QueueMessage> = [
    {
      title: 'Message ID',
      dataIndex: 'messageId',
      key: 'messageId',
      width: 300,
      ellipsis: true,
      render: (id: string) => (
        <Text code copyable>
          {id}
        </Text>
      ),
    },
    {
      title: 'Sent Time',
      key: 'sentTime',
      width: 200,
      render: (_, record) => (
        <Text>{formatTimestamp(record.attributes.SentTimestamp)}</Text>
      ),
    },
    {
      title: 'Receive Count',
      key: 'receiveCount',
      width: 120,
      align: 'center',
      render: (_, record) => {
        const count = parseInt(
          record.attributes.ApproximateReceiveCount || '0'
        );
        return (
          <Badge
            count={count}
            showZero
            style={{ backgroundColor: count > 1 ? '#ff4d4f' : '#52c41a' }}
          />
        );
      },
    },
    {
      title: 'Preview',
      key: 'preview',
      ellipsis: true,
      render: (_, record) => {
        const body = parseMessageBody(record.body);
        const preview =
          typeof body === 'string'
            ? body
            : JSON.stringify(body).substring(0, 100);
        return <Text type="secondary">{preview}...</Text>;
      },
    },
  ];

  const expandedRowRender = (record: QueueMessage) => {
    const body = parseMessageBody(record.body);
    const isJson = typeof body === 'object';

    return (
      <div className="bg-gray-50 p-4 rounded">
        <Space direction="vertical" style={{ width: '100%' }} size="large">
          {/* Message Details */}
          <Descriptions title="Message Details" bordered size="small" column={2}>
            <Descriptions.Item label="Message ID" span={2}>
              <Text code copyable>
                {record.messageId}
              </Text>
            </Descriptions.Item>
            <Descriptions.Item label="Receipt Handle" span={2}>
              <Text code copyable ellipsis>
                {record.receiptHandle}
              </Text>
            </Descriptions.Item>
            {record.md5OfBody && (
              <Descriptions.Item label="MD5 of Body" span={2}>
                <Text code>{record.md5OfBody}</Text>
              </Descriptions.Item>
            )}
            <Descriptions.Item label="Sent Timestamp">
              {formatTimestamp(record.attributes.SentTimestamp)}
            </Descriptions.Item>
            <Descriptions.Item label="First Receive">
              {formatTimestamp(
                record.attributes.ApproximateFirstReceiveTimestamp
              )}
            </Descriptions.Item>
            <Descriptions.Item label="Receive Count">
              {record.attributes.ApproximateReceiveCount || '0'}
            </Descriptions.Item>
          </Descriptions>

          {/* Message Body */}
          <div>
            <Title level={5}>Message Body</Title>
            <Card bodyStyle={{ padding: 0 }}>
              <Paragraph
                code
                copyable
                style={{
                  margin: 0,
                  padding: '12px',
                  backgroundColor: '#1e1e1e',
                  color: '#d4d4d4',
                  maxHeight: '400px',
                  overflow: 'auto',
                }}
              >
                <pre style={{ margin: 0, fontFamily: 'monospace' }}>
                  {isJson ? formatJson(body) : record.body}
                </pre>
              </Paragraph>
            </Card>
          </div>

          {/* Message Attributes */}
          {record.messageAttributes &&
            Object.keys(record.messageAttributes).length > 0 && (
              <div>
                <Title level={5}>Message Attributes</Title>
                <Card bodyStyle={{ padding: 0 }}>
                  <Paragraph
                    code
                    copyable
                    style={{
                      margin: 0,
                      padding: '12px',
                      backgroundColor: '#1e1e1e',
                      color: '#d4d4d4',
                    }}
                  >
                    <pre style={{ margin: 0, fontFamily: 'monospace' }}>
                      {formatJson(record.messageAttributes)}
                    </pre>
                  </Paragraph>
                </Card>
              </div>
            )}
        </Space>
      </div>
    );
  };

  if (!name) {
    return (
      <div className="p-6">
        <Card>
          <Text type="danger">Queue name is required</Text>
        </Card>
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mb-6">
        <Title level={2}>
          <InboxOutlined className="mr-2" />
          Queue Details
        </Title>
        <Text type="secondary">
          Viewing messages for queue: <Text strong>{name}</Text>
        </Text>
      </div>

      {/* Queue Info Card */}
      {queueInfo && (
        <Card className="mb-4" title="Queue Information">
          <Row gutter={16}>
            <Col xs={24} sm={12} md={6}>
              <Statistic
                title="Queue Type"
                value={queueTypeConfig[queueInfo.type].label}
                prefix={queueTypeConfig[queueInfo.type].icon}
              />
            </Col>
            <Col xs={24} sm={12} md={6}>
              <Statistic
                title="Messages"
                value={queueInfo.messageCount}
                prefix={<MessageOutlined />}
              />
            </Col>
            <Col xs={24} sm={12} md={6}>
              <Statistic
                title="In Flight"
                value={queueInfo.messagesInFlight}
                prefix={<SendOutlined />}
              />
            </Col>
            <Col xs={24} sm={12} md={6}>
              <Statistic
                title="Oldest Message"
                value={formatAge(queueInfo.oldestMessageAge)}
                prefix={<ClockCircleOutlined />}
              />
            </Col>
          </Row>
          {queueInfo.url && (
            <div className="mt-4">
              <Text type="secondary">Queue URL: </Text>
              <Text code copyable>
                {queueInfo.url}
              </Text>
            </div>
          )}
        </Card>
      )}

      {/* Actions */}
      <Card className="mb-4">
        <Row justify="space-between" align="middle">
          <Col>
            <Space>
              <Text strong>
                {messages.length} message{messages.length !== 1 ? 's' : ''}{' '}
                found
              </Text>
            </Space>
          </Col>
          <Col>
            <Button
              icon={<ReloadOutlined />}
              onClick={() => refetch()}
              loading={isLoading}
              size="large"
            >
              Refresh Messages
            </Button>
          </Col>
        </Row>
      </Card>

      {/* Messages Table */}
      <Card>
        <Table
          dataSource={messages}
          columns={columns}
          rowKey="messageId"
          loading={isLoading}
          expandable={{
            expandedRowRender,
            expandedRowKeys,
            onExpandedRowsChange: (keys) =>
              setExpandedRowKeys(keys as string[]),
            expandIcon: ({ expanded, onExpand, record }) => (
              <Button
                type="link"
                size="small"
                onClick={(e) => onExpand(record, e)}
              >
                {expanded ? 'Collapse' : 'Expand'}
              </Button>
            ),
          }}
          pagination={{
            pageSize: 10,
            showSizeChanger: true,
            showTotal: (total) => `Total ${total} messages`,
          }}
          locale={{
            emptyText: 'No messages in this queue',
          }}
        />
      </Card>
    </div>
  );
};
