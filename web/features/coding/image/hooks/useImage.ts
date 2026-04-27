import React from 'react';
import { useImageStore } from '../stores/imageStore';

export const useImage = () => {
  const {
    channels,
    jobs,
    loading,
    submitting,
    channelSaving,
    activeView,
    editingChannelId,
    lastJobId,
    loadWorkspace,
    refreshJobs,
    saveChannel,
    removeChannel,
    removeJob,
    reorderChannels,
    submitJob,
    setActiveView,
    setEditingChannelId,
  } = useImageStore();

  const hasLoadedRef = React.useRef(false);

  React.useEffect(() => {
    if (hasLoadedRef.current) return;
    hasLoadedRef.current = true;
    void loadWorkspace();
  }, [loadWorkspace]);

  const latestJob = React.useMemo(
    () => jobs.find((job) => job.id === lastJobId) ?? jobs[0] ?? null,
    [jobs, lastJobId]
  );

  return {
    channels,
    jobs,
    latestJob,
    loading,
    submitting,
    channelSaving,
    activeView,
    editingChannelId,
    loadWorkspace,
    refreshJobs,
    saveChannel,
    removeChannel,
    removeJob,
    reorderChannels,
    submitJob,
    setActiveView,
    setEditingChannelId,
  };
};

export default useImage;
