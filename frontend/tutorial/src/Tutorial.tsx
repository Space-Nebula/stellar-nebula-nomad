import React, { useState, useEffect } from 'react';

interface TutorialStep {
  id: number;
  title: string;
  description: string;
  reward: number;
  completed: boolean;
}

interface TutorialProgress {
  next_step: number;
  completed_mask: number;
  completed_count: number;
  started_at: number;
  completed_at: number;
}

const TOTAL_STEPS = 10;
const STEP_REWARDS = [25, 35, 50, 65, 100, 125, 150, 200, 250, 500];

const TUTORIAL_STEPS: TutorialStep[] = [
  { id: 0, title: "Welcome to Nebula Nomad", description: "Learn the basics of space exploration and cosmic discovery.", reward: 25, completed: false },
  { id: 1, title: "Scan Your First Nebula", description: "Use your ship to scan a nebula and discover its secrets.", reward: 35, completed: false },
  { id: 2, title: "Harvest Resources", description: "Collect cosmic essence and valuable resources from nebulas.", reward: 50, completed: false },
  { id: 3, title: "Mint Your Ship NFT", description: "Create your unique ship NFT to explore the cosmos.", reward: 65, completed: false },
  { id: 4, title: "Join a Guild", description: "Connect with other explorers and form alliances.", reward: 100, completed: false },
  { id: 5, title: "Complete Daily Missions", description: "Take on daily challenges to earn extra rewards.", reward: 125, completed: false },
  { id: 6, title: "Explore the Marketplace", description: "Buy and sell resources with other players.", reward: 150, completed: false },
  { id: 7, title: "Upgrade Your Ship", description: "Enhance your ship with powerful upgrades.", reward: 200, completed: false },
  { id: 8, title: "Enter the Leaderboards", description: "Compete with others and climb the rankings.", reward: 250, completed: false },
  { id: 9, title: "Master of the Cosmos", description: "Complete all tutorial steps and become a master explorer!", reward: 500, completed: false },
];

const Tutorial: React.FC = () => {
  const [progress, setProgress] = useState<TutorialProgress | null>(null);
  const [steps, setSteps] = useState<TutorialStep[]>(TUTORIAL_STEPS);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadTutorialProgress();
  }, []);

  const loadTutorialProgress = async () => {
    setLoading(true);
    try {
      // TODO: Replace with actual Soroban contract call
      // const result = await contract.get_tutorial_progress(playerAddress);
      // For now, simulate with default progress
      const simulatedProgress: TutorialProgress = {
        next_step: 0,
        completed_mask: 0,
        completed_count: 0,
        started_at: Date.now(),
        completed_at: 0,
      };
      setProgress(simulatedProgress);
      updateStepsFromProgress(simulatedProgress);
    } catch (error) {
      console.error("Failed to load tutorial progress:", error);
    } finally {
      setLoading(false);
    }
  };

  const updateStepsFromProgress = (prog: TutorialProgress) => {
    const updatedSteps = TUTORIAL_STEPS.map((step) => ({
      ...step,
      completed: (prog.completed_mask & (1 << step.id)) !== 0,
    }));
    setSteps(updatedSteps);
  };

  const handleStartTutorial = async () => {
    try {
      // TODO: Call contract.start_tutorial(playerAddress)
      alert("Tutorial started! Complete steps to earn rewards.");
      loadTutorialProgress();
    } catch (error) {
      console.error("Failed to start tutorial:", error);
    }
  };

  const handleCompleteStep = async (stepId: number) => {
    try {
      // TODO: Call contract.complete_tutorial_step(playerAddress, stepId)
      alert(`Step ${stepId + 1} completed! You earned ${STEP_REWARDS[stepId]} cosmic essence.`);
      loadTutorialProgress();
    } catch (error) {
      console.error("Failed to complete step:", error);
    }
  };

  const getTotalRewards = () => {
    return steps
      .filter((step) => step.completed)
      .reduce((sum, step) => sum + step.reward, 0);
  };

  if (loading) {
    return (
      <div style={{ padding: '20px', textAlign: 'center' }}>
        <h2>Loading Tutorial...</h2>
      </div>
    );
  }

  return (
    <div style={{ maxWidth: '800px', margin: '0 auto', padding: '20px' }}>
      <h1 style={{ color: '#4a90e2' }}>🚀 Interactive Tutorial</h1>
      
      {!progress && (
        <div style={{ marginBottom: '20px', padding: '15px', backgroundColor: '#f0f8ff', borderRadius: '8px' }}>
          <p>Welcome to Nebula Nomad! Start the tutorial to learn the game and earn rewards.</p>
          <button
            onClick={handleStartTutorial}
            style={{
              padding: '10px 20px',
              fontSize: '16px',
              backgroundColor: '#4a90e2',
              color: 'white',
              border: 'none',
              borderRadius: '5px',
              cursor: 'pointer',
            }}
          >
            Start Tutorial
          </button>
        </div>
      )}

      {progress && (
        <div style={{ marginBottom: '20px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '10px' }}>
            <span>Progress: {progress.completed_count}/{TOTAL_STEPS} steps</span>
            <span>Rewards Earned: {getTotalRewards()} essence</span>
          </div>
          <div style={{ width: '100%', height: '10px', backgroundColor: '#e0e0e0', borderRadius: '5px' }}>
            <div
              style={{
                width: `${(progress.completed_count / TOTAL_STEPS) * 100}%`,
                height: '100%',
                backgroundColor: '#4a90e2',
                borderRadius: '5px',
              }}
            />
          </div>
        </div>
      )}

      <div style={{ display: 'grid', gap: '15px' }}>
        {steps.map((step) => (
          <div
            key={step.id}
            style={{
              padding: '15px',
              border: '1px solid #ddd',
              borderRadius: '8px',
              backgroundColor: step.completed ? '#e8f5e9' : 'white',
              opacity: step.id > (progress?.next_step || 0) && !step.completed ? 0.5 : 1,
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <h3 style={{ margin: '0 0 10px 0' }}>
                  {step.completed ? '✅' : `Step ${step.id + 1}:`} {step.title}
                </h3>
                <p style={{ margin: '0', color: '#666' }}>{step.description}</p>
              </div>
              <div style={{ textAlign: 'right' }}>
                <div style={{ color: '#4a90e2', fontWeight: 'bold' }}>{step.reward} essence</div>
                {!step.completed && progress && step.id === progress.next_step && (
                  <button
                    onClick={() => handleCompleteStep(step.id)}
                    style={{
                      marginTop: '10px',
                      padding: '5px 15px',
                      backgroundColor: '#4caf50',
                      color: 'white',
                      border: 'none',
                      borderRadius: '3px',
                      cursor: 'pointer',
                    }}
                  >
                    Complete
                  </button>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>

      {progress && progress.completed_count === TOTAL_STEPS && (
        <div style={{ marginTop: '20px', padding: '20px', backgroundColor: '#fff3e0', borderRadius: '8px', textAlign: 'center' }}>
          <h2>🎉 Tutorial Complete!</h2>
          <p>You've mastered the basics of Nebula Nomad. Enjoy your cosmic journey!</p>
          <p>Total Rewards Earned: {getTotalRewards()} cosmic essence</p>
        </div>
      )}

      <div style={{ marginTop: '30px', padding: '15px', backgroundColor: '#f5f5f5', borderRadius: '8px' }}>
        <h3>Skip Option for Veterans</h3>
        <p>If you're an experienced player, you can skip the tutorial and receive starter resources.</p>
        <button
          style={{
            padding: '10px 20px',
            backgroundColor: '#ff9800',
            color: 'white',
            border: 'none',
            borderRadius: '5px',
            cursor: 'pointer',
          }}
        >
          Skip Tutorial (Get Starter Resources)
        </button>
      </div>
    </div>
  );
};

export default Tutorial;
