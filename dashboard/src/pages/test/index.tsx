import { useState } from 'react';
import { useCustom } from '@refinedev/core';
import { useNavigate } from 'react-router-dom';
import { Card, Tabs, Form, Input, Button, Select, message, Alert, Table, Upload } from 'antd';
import { SendOutlined, UploadOutlined, FileSearchOutlined } from '@ant-design/icons';
import { apiClient } from '../../utils/api';

const { TextArea } = Input;

export const TestEmailPage = () => {
  const [result, setResult] = useState<any>(null);
  const [attachments, setAttachments] = useState<any[]>([]);
  const navigate = useNavigate();

  const { query: historyQuery } = useCustom({
    url: '/test/history',
    method: 'get',
  });

  const { data: historyData, refetch } = historyQuery;

  const [sendingInbound, setSendingInbound] = useState(false);
  const [sendingOutbound, setSendingOutbound] = useState(false);

  const handleFileUpload = (file: File) => {
    // Check total size - limit to 7MB to account for base64 encoding overhead (~33%)
    // 7MB * 1.33 = 9.31MB, leaving room for JSON structure under API Gateway's 10MB limit
    const totalSize = attachments.reduce((sum, att) => sum + att.size, 0) + file.size;
    if (totalSize > 7 * 1024 * 1024) {
      message.error('Total attachment size cannot exceed 7 MB (API Gateway limit)');
      return false;
    }

    // Convert to base64
    const reader = new FileReader();
    reader.onload = (e) => {
      const base64 = e.target?.result?.toString().split(',')[1];
      setAttachments([
        ...attachments,
        {
          filename: file.name,
          contentType: file.type || 'application/octet-stream',
          data: base64,
          size: file.size,
        },
      ]);
    };
    reader.readAsDataURL(file);
    return false; // Prevent default upload
  };

  const onSendInbound = async (values: any) => {
    setSendingInbound(true);
    try {
      const payload = {
        ...values,
        attachments: attachments.length > 0 ? attachments : undefined,
      };
      const response = await apiClient.post('/test/inbound', payload);
      setResult(response.data);
      message.success('Test email sent successfully!');
      setAttachments([]);
      refetch();
    } catch (error: any) {
      message.error(error.message || 'Failed to send test email');
    } finally {
      setSendingInbound(false);
    }
  };

  const onSendOutbound = async (values: any) => {
    setSendingOutbound(true);
    try {
      const response = await apiClient.post('/test/outbound', values);
      setResult(response.data);
      message.success('Test email queued successfully!');
      refetch();
    } catch (error: any) {
      message.error(error.message || 'Failed to queue test email');
    } finally {
      setSendingOutbound(false);
    }
  };

  const historyColumns = [
    { title: 'Timestamp', dataIndex: 'timestamp', key: 'timestamp', width: 180 },
    { title: 'Type', dataIndex: 'type', key: 'type', width: 100 },
    { title: 'Recipient', dataIndex: 'recipient', key: 'recipient' },
    { title: 'Status', dataIndex: 'status', key: 'status', width: 100 },
    { title: 'Message ID', dataIndex: 'messageId', key: 'messageId', width: 200 },
    {
      title: 'Actions',
      key: 'actions',
      width: 120,
      render: (_: any, record: any) => (
        <Button
          size="small"
          icon={<FileSearchOutlined />}
          onClick={() => navigate(`/logs?messageId=${record.messageId}`)}
        >
          View Logs
        </Button>
      ),
    },
  ];

  return (
    <div className="p-6 space-y-6">
      <h1 className="text-2xl font-bold">Test Email</h1>

      {result && (
        <Alert
          message={result.success ? 'Success' : 'Error'}
          description={result.success ? `Message ID: ${result.messageId}` : result.error}
          type={result.success ? 'success' : 'error'}
          closable
          onClose={() => setResult(null)}
        />
      )}

      <Card>
        <Tabs
          items={[
            {
              key: 'inbound',
              label: 'Inbound Test',
              children: (
                <Form layout="vertical" onFinish={onSendInbound}>
                  <Form.Item label="App" name="app" rules={[{ required: true }]}>
                    <Select>
                      <Select.Option value="app1">app1</Select.Option>
                      <Select.Option value="app2">app2</Select.Option>
                    </Select>
                  </Form.Item>
                  <Form.Item label="From" name="from" rules={[{ required: true, type: 'email' }]}>
                    <Input placeholder="test@example.com" />
                  </Form.Item>
                  <Form.Item label="Subject" name="subject" rules={[{ required: true }]}>
                    <Input placeholder="Test Subject" />
                  </Form.Item>

                  <Tabs
                    defaultActiveKey="text"
                    items={[
                      {
                        key: 'text',
                        label: 'Text',
                        children: (
                          <Form.Item name={['body', 'text']} rules={[{ required: true }]}>
                            <TextArea rows={6} placeholder="Plain text email body..." />
                          </Form.Item>
                        ),
                      },
                      {
                        key: 'html',
                        label: 'HTML',
                        children: (
                          <Form.Item name={['body', 'html']}>
                            <TextArea rows={6} placeholder="<html>...</html>" />
                          </Form.Item>
                        ),
                      },
                    ]}
                  />

                  <Form.Item label="Attachments">
                    <Upload
                      beforeUpload={handleFileUpload}
                      fileList={attachments.map((att, idx) => ({
                        uid: idx.toString(),
                        name: att.filename,
                        status: 'done',
                        size: att.size,
                      }))}
                      onRemove={(file) => {
                        const index = parseInt(file.uid);
                        setAttachments(attachments.filter((_, i) => i !== index));
                      }}
                    >
                      <Button icon={<UploadOutlined />}>Upload Attachment</Button>
                    </Upload>
                    <div className="text-sm text-gray-500 mt-2">
                      Max total size: 7 MB (due to base64 encoding + API Gateway 10MB limit). Current: {(attachments.reduce((sum, att) => sum + att.size, 0) / 1024 / 1024).toFixed(2)} MB
                    </div>
                  </Form.Item>

                  <Button type="primary" htmlType="submit" loading={sendingInbound} icon={<SendOutlined />}>
                    Send Test Email
                  </Button>
                </Form>
              ),
            },
            {
              key: 'outbound',
              label: 'Outbound Test',
              children: (
                <Form layout="vertical" onFinish={onSendOutbound}>
                  <Form.Item label="From App" name="from" rules={[{ required: true }]}>
                    <Select>
                      <Select.Option value="app1">app1</Select.Option>
                      <Select.Option value="app2">app2</Select.Option>
                    </Select>
                  </Form.Item>
                  <Form.Item label="To" name="to" rules={[{ required: true, type: 'email' }]}>
                    <Input placeholder="recipient@example.com" />
                  </Form.Item>
                  <Form.Item label="Subject" name="subject" rules={[{ required: true }]}>
                    <Input placeholder="Test Subject" />
                  </Form.Item>

                  <Tabs
                    defaultActiveKey="text"
                    items={[
                      {
                        key: 'text',
                        label: 'Text',
                        children: (
                          <Form.Item name={['body', 'text']} rules={[{ required: true }]}>
                            <TextArea rows={6} placeholder="Plain text email body..." />
                          </Form.Item>
                        ),
                      },
                      {
                        key: 'html',
                        label: 'HTML',
                        children: (
                          <Form.Item name={['body', 'html']}>
                            <TextArea rows={6} placeholder="<html>...</html>" />
                          </Form.Item>
                        ),
                      },
                    ]}
                  />

                  <Button type="primary" htmlType="submit" loading={sendingOutbound} icon={<SendOutlined />}>
                    Queue Test Email
                  </Button>
                </Form>
              ),
            },
          ]}
        />
      </Card>

      <Card title="Test History">
        <Table
          dataSource={historyData?.data?.tests || []}
          columns={historyColumns}
          rowKey="id"
        />
      </Card>
    </div>
  );
};
