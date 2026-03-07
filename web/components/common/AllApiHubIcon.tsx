import React from 'react';
import allApiHubIcon from '@/assets/all-api-hub.png';

interface AllApiHubIconProps {
  size?: number;
  className?: string;
  alt?: string;
}

const AllApiHubIcon: React.FC<AllApiHubIconProps> = ({
  size = 16,
  className,
  alt = 'All API Hub',
}) => {
  return (
    <img
      src={allApiHubIcon}
      alt={alt}
      width={size}
      height={size}
      className={className}
      style={{ display: 'block', borderRadius: 4, objectFit: 'cover' }}
    />
  );
};

export default AllApiHubIcon;
