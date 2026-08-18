"use client";

import React, { useState, useEffect } from "react";

export function MiniLoader() {
  const [slideIndex, setSlideIndex] = useState(0);
  const [hasStarted, setHasStarted] = useState(false);

  useEffect(() => {
    const timer = setInterval(() => {
      setSlideIndex((prev) => (prev + 1) % 5);
      setHasStarted(true);
    }, 750);
    return () => clearInterval(timer);
  }, []);

  const renderIconContent = () => {
    switch (slideIndex) {
      case 0:
        return (
          <div className={`w-full h-full flex items-center justify-center bg-rose-500 text-white ${hasStarted ? 'animate-in slide-in-from-right duration-500' : ''}`}>
            <i className="fa-solid fa-heart text-xl"></i>
          </div>
        );

      case 1:
        return (
          <div className="w-full h-full flex items-center justify-center bg-indigo-600 text-white animate-in duration-500">
            <span className="font-black text-xl">NG</span>
          </div>
        );

      case 2:
        return (
          <div className="w-full h-full flex items-center justify-center bg-sky-500 text-white animate-in slide-in-from-right duration-500">
            <i className="fa-solid fa-user-group text-xl"></i>
          </div>
        );

      case 3:
        return (
          <div className="w-full h-full flex items-center justify-center bg-emerald-500 text-white animate-in slide-in-from-right duration-500">
            <i className="fa-solid fa-chalkboard-user text-xl"></i>
          </div>
        );

      case 4:
        return (
          <div className="w-full h-full flex items-center justify-center bg-amber-500 text-white animate-in slide-in-from-right duration-500">
            <i className="fa-solid fa-seedling text-xl"></i>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="w-14 h-14 rounded-lg mx-auto overflow-hidden shadow-xl shadow-indigo-600/30 mb-5 transition-transform hover:scale-105 bg-white dark:bg-slate-900 relative">
      {/* Spinning Rings */}
      <div className="absolute -inset-1.5 rounded-md border-2 border-slate-100 dark:border-slate-800"></div>
      <div className={`absolute -inset-1.5 rounded-md border-2 border-t-transparent animate-spin transition-colors duration-500 ${slideIndex === 0 ? 'border-rose-500' :
        slideIndex === 1 ? 'border-indigo-600' :
          slideIndex === 2 ? 'border-sky-500' :
            slideIndex === 3 ? 'border-emerald-500' :
              'border-amber-500'
        }`}></div>
      <div className="relative z-10 w-full h-full">
        {renderIconContent()}
      </div>
    </div>
  );
}
