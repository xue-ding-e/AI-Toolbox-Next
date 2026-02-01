import React from 'react';
import { Empty } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  sortableKeyboardCoordinates,
  rectSortingStrategy,
} from '@dnd-kit/sortable';
import { restrictToWindowEdges } from '@dnd-kit/modifiers';
import { SkillCard } from './SkillCard';
import type { ManagedSkill, ToolOption } from '../types';
import styles from './SkillsList.module.less';

interface SkillsListProps {
  skills: ManagedSkill[];
  allTools: ToolOption[];
  loading: boolean;
  getGithubInfo: (url: string | null | undefined) => { label: string; href: string } | null;
  getSkillSourceLabel: (skill: ManagedSkill) => string;
  formatRelative: (ms: number | null | undefined) => string;
  onUpdate: (skill: ManagedSkill) => void;
  onDelete: (skillId: string) => void;
  onToggleTool: (skill: ManagedSkill, toolId: string) => void;
  onDragEnd: (event: DragEndEvent) => void;
}

export const SkillsList: React.FC<SkillsListProps> = ({
  skills,
  allTools,
  loading,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onToggleTool,
  onDragEnd,
}) => {
  const { t } = useTranslation();

  // Configure drag sensors
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // Prevent accidental drags
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  if (skills.length === 0) {
    return (
      <div className={styles.empty}>
        <Empty description={t('skills.skillsEmpty')} />
      </div>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={[restrictToWindowEdges]}
      onDragEnd={onDragEnd}
    >
      <SortableContext
        items={skills.map((s) => s.id)}
        strategy={rectSortingStrategy}
      >
        <div className={styles.list}>
          {skills.map((skill) => (
            <SkillCard
              key={skill.id}
              skill={skill}
              allTools={allTools}
              loading={loading}
              getGithubInfo={getGithubInfo}
              getSkillSourceLabel={getSkillSourceLabel}
              formatRelative={formatRelative}
              onUpdate={onUpdate}
              onDelete={onDelete}
              onToggleTool={onToggleTool}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
};
