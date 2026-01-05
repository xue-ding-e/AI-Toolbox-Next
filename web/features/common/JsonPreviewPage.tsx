import React from 'react';
import { Button, Typography } from 'antd';
import { ArrowLeftOutlined } from '@ant-design/icons';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { usePreviewStore } from '@/stores';
import JsonEditor from '@/components/common/JsonEditor';

const { Title } = Typography;

const JsonPreviewPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { title, data, returnPath, clearPreviewData } = usePreviewStore();
  const containerRef = React.useRef<HTMLDivElement>(null);
  const [editorHeight, setEditorHeight] = React.useState(500);

  const returnPathRef = React.useRef(returnPath);

  React.useEffect(() => {
    if (data === null) {
      const path = returnPathRef.current || '/coding/claudecode';
      navigate(path, { replace: true });
    }
  }, [data, navigate]);

  React.useEffect(() => {
    const updateHeight = () => {
      if (containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        setEditorHeight(rect.height - 80);
      }
    };

    updateHeight();
    window.addEventListener('resize', updateHeight);
    const observer = new ResizeObserver(updateHeight);
    if (containerRef.current) {
      observer.observe(containerRef.current);
    }
    return () => {
      window.removeEventListener('resize', updateHeight);
      observer.disconnect();
    };
  }, []);

  const handleBack = () => {
    const path = returnPathRef.current || '/coding/claudecode';
    clearPreviewData();
    navigate(path, { replace: true });
  };

  if (data === null) {
    return null;
  }

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: 16, boxSizing: 'border-box' }} ref={containerRef}>
      <div style={{ marginBottom: 16, display: 'flex', alignItems: 'center', gap: 16, flexShrink: 0 }}>
        <Button icon={<ArrowLeftOutlined />} onClick={handleBack}>
          {t('common.back')}
        </Button>
        <Title level={4} style={{ margin: 0 }}>
          {title || t('common.previewConfig')}
        </Title>
      </div>

      <div style={{ flex: 1, minHeight: 0 }}>
        <JsonEditor
          value={data}
          readOnly={true}
          mode="text"
          height={editorHeight}
        />
      </div>
    </div>
  );
};

export default JsonPreviewPage;
