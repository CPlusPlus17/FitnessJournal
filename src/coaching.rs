use chrono::{Duration, Utc};
use crate::models::{TrainingPlan, TrainingTarget, WorkoutType};

pub struct CoachContext {
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub available_equipment: Vec<String>,
}

pub struct Coach;

impl Coach {
    pub fn new() -> Self {
        Coach
    }

    #[allow(dead_code)]
    pub fn generate_smart_plan(&self, history: &[crate::models::ActivitySummary], detailed_activities: &[crate::models::GarminActivity]) -> TrainingPlan {
        let now = Utc::now();
        let week_start = now - Duration::days(7);
        
        // Analyze recent history (last 7 days)
        let recent_activities: Vec<&crate::models::ActivitySummary> = history.iter()
            .filter(|a| a.time > week_start)
            .collect();
            
        let bike_count = recent_activities.iter().filter(|a| {
            let s = a.sport.to_lowercase();
            s.contains("cycling") || s.contains("biking")
        }).count();
        
        let run_count = recent_activities.iter().filter(|a| a.sport.to_lowercase().contains("running")).count();
        
        let strength_count = recent_activities.iter().filter(|a| {
            let s = a.sport.to_lowercase();
            s.contains("strength") || s.contains("fitness")
        }).count();

        // Analyze Strength Volume from Detailed Data
        let mut strength_volume_kg = 0.0;
        let week_start_str = week_start.format("%Y-%m-%dT%H:%M:%S").to_string();
        for da in detailed_activities {
            if da.start_time > week_start_str { 
                 if let Some(crate::models::GarminSetsData::Details(data)) = &da.sets {
                     let vol: f64 = data.exercise_sets.iter()
                         .filter(|s| s.set_type == "ACTIVE")
                         .map(|s| s.weight.unwrap_or(0.0) / 1000.0 * (s.repetition_count.unwrap_or(0) as f64))
                         .sum();
                     strength_volume_kg += vol;
                 }
            }
        }
        
        println!("Recent Activity (Last 7d): Bike: {}, Run: {}, Strength: {} (Vol: {:.0}kg)", 
            bike_count, run_count, strength_count, strength_volume_kg);

        let mut workouts = Vec::new();
        let end_of_week = now + Duration::days(7);

        // --- Biking Logic ---
        if bike_count < 1 {
            // Boot up / Base Phase
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 45.0,
                target_distance_km: None,
                description: "Base Builder: Easy spin to get back into rhythm. Zone 1-2.".to_string(),
            });
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 60.0,
                target_distance_km: None,
                description: "Endurance: Steady ride, focus on cadence.".to_string(),
            });
        } else {
            // Progression
            workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 60.0,
                target_distance_km: None,
                description: "Hill Repeats: 4x 5min climbing at threshold.".to_string(),
            });
             workouts.push(TrainingTarget {
                workout_type: WorkoutType::Bike,
                target_duration_minutes: 90.0,
                target_distance_km: None,
                description: "Mountain Endurance: Long steady climb simulation.".to_string(),
            });
        }

        // --- Strength Logic ---
        // Volume check for coaching advice
        let strength_focus = if strength_volume_kg > 5000.0 {
            "Deload / Technique Focus: Keep weights light, focus on mobility."
        } else {
             "Progression: Aim to increase weight or reps."
        };

        if strength_count < 2 {
             workouts.push(TrainingTarget {
                workout_type: WorkoutType::Strength,
                target_duration_minutes: 45.0,
                target_distance_km: None,
                description: format!("Full Body A: Squats, Pushups, Rows, Core. {}", strength_focus),
            });
        }
        workouts.push(TrainingTarget {
            workout_type: WorkoutType::Strength,
            target_duration_minutes: 45.0,
            target_distance_km: None,
            description: format!("Full Body B: Deadlifts, Overhead Press, Lunges. {}", strength_focus),
        });

        // --- Running Note ---
        // We don't schedule running (Garmin Coach does), but we acknowledge it.
        if run_count > 2 {
             workouts.push(TrainingTarget {
                workout_type: WorkoutType::Unknown,
                target_duration_minutes: 0.0,
                target_distance_km: None,
                description: "Note: High running volume detected. Ensure bike rides are low impact.".to_string(),
            });
        }

        TrainingPlan {
            start_date: now,
            end_date: end_of_week,
            workouts,
        }
    }

    pub fn generate_brief(
        &self, 
        history: &[crate::models::ActivitySummary], 
        detailed_activities: &[crate::models::GarminActivity], 
        plans: &[crate::models::GarminPlan],
        profile: &Option<crate::models::GarminProfile>,
        metrics: &Option<crate::models::GarminMaxMetrics>,
        scheduled_workouts: &[crate::models::ScheduledWorkout],
        recovery_metrics: &Option<crate::models::GarminRecoveryMetrics>,
        context: &CoachContext
    ) -> String {
        let now = Utc::now();
        let mut brief = String::new();

        // 1. Header & Current Context
        brief.push_str("# Certified Coaching Brief\n\n");
        brief.push_str("**Role**: You are an elite Multi-Sport Coach (Triathlon/Strength/Endurance). Your job is to analyze the athlete's data and produce a highly specific, periodized training plan.\n\n");
        
        let today_date_str = now.format("%Y-%m-%d").to_string();
        brief.push_str(&format!("**Current Date**: {}\n\n", today_date_str));

        // Let's summarize what was already done today from the history
        brief.push_str("**Activities Completed Today**:\n");
        let todays_activities: Vec<&crate::models::ActivitySummary> = history.iter()
            .filter(|a| a.time.format("%Y-%m-%d").to_string() == today_date_str)
            .collect();
        
        if todays_activities.is_empty() {
            brief.push_str("- None.\n\n");
        } else {
            for a in todays_activities {
                brief.push_str(&format!("- **{}**: {:.1} min, {:.1} km\n", a.name, a.duration_minutes, a.distance_km));
            }
            brief.push_str("\n");
        }

        if let Some(rec) = recovery_metrics {
            brief.push_str("**Today's Recovery & Readiness**:\n");
            if let Some(bb) = rec.current_body_battery {
                brief.push_str(&format!("- **Body Battery**: {} / 100\n", bb));
            }
            if let Some(ss) = rec.sleep_score {
                brief.push_str(&format!("- **Sleep Score**: {} / 100\n", ss));
            }
            brief.push_str("\n");
        }

        // 2. Athlete Profile
        brief.push_str("## 1. Athlete Profile\n");
        if let Some(p) = profile {
            if let Some(w) = p.weight { brief.push_str(&format!("- **Weight**: {:.1} kg\n", w / 1000.0)); } // Weight is in grams usually? Check Garmin output. Output says 72500.0, so yes grams.
            if let Some(h) = p.height { brief.push_str(&format!("- **Height**: {:.1} cm\n", h)); }
            if let Some(dob) = &p.birth_date { brief.push_str(&format!("- **DOB**: {}\n", dob)); }
            if let Some(v) = p.vo2_max_running { brief.push_str(&format!("- **VO2Max (Run)**: {:.1}\n", v)); }
        }
        if let Some(m) = metrics {
             if let Some(v) = m.vo2_max_precise { brief.push_str(&format!("- **VO2Max (Precise)**: {:.1}\n", v)); }
             if let Some(fa) = m.fitness_age { brief.push_str(&format!("- **Fitness Age**: {}\n", fa)); }
        }
        brief.push_str("\n");

        // 3. Goals & Constraints
        brief.push_str("## 2. Goals & Context\n");
        brief.push_str("**Primary Goals**:\n");
        for g in &context.goals {
            brief.push_str(&format!("- [ ] {}\n", g));
        }
        
        brief.push_str("\n**Available Equipment**:\n");
        for e in &context.available_equipment {
            brief.push_str(&format!("- {}\n", e));
        }

        brief.push_str("\n**Active Training Cycles (Garmin Coach)**:\n");
        if plans.is_empty() {
            brief.push_str("- None active.\n");
        } else {
            for p in plans {
                brief.push_str(&format!("- **{}** (Type: {}, Ends: {})\n", p.name, p.plan_type, p.end_date));
            }
        }

        brief.push_str("\n**Scheduled Garmin Workouts**:\n");
        if scheduled_workouts.is_empty() {
            brief.push_str("- None scheduled.\n");
        } else {
            for sw in scheduled_workouts {
                let mut details = format!("- **{}** (Date: {}, Sport: {}", sw.title, sw.date, sw.sport);
                if let Some(d) = sw.duration { details.push_str(&format!(", Duration: {:.0}min", d)); }
                if let Some(dist) = sw.distance { details.push_str(&format!(", Distance: {:.1}km", dist)); }
                if let Some(desc) = &sw.description { details.push_str(&format!(", Focus: '{}'", desc)); }
                details.push_str(")\n");
                brief.push_str(&details);
            }
            brief.push_str("\n*Note for Coach*: Please consider the scheduled Garmin workouts above. Advise if today's scheduled workout should be performed, and adjust the strength volume if necessary.\n");
        }
        brief.push_str("\n**Constraints**:\n");
        for c in &context.constraints {
            brief.push_str(&format!("- {}\n", c));
        }
        brief.push_str("\n");

        // 4. Status Update (30 Days)
        let _total_count = history.len();
        let total_dist_km: f64 = history.iter().map(|a| a.distance_km).sum();
        let total_dur_min: f64 = history.iter().map(|a| a.duration_minutes).sum();
        
        brief.push_str("## 3. Training Status (Last 30 Days)\n");
        brief.push_str(&format!("- **Volume**: {:.1} km / {:.1} hours\n", total_dist_km, total_dur_min / 60.0));
        
        let run_count = history.iter().filter(|a| a.sport.to_lowercase().contains("run")).count();
        let bike_count = history.iter().filter(|a| { let s = a.sport.to_lowercase(); s.contains("bike") || s.contains("cycl") }).count();
        let strength_count = history.iter().filter(|a| a.sport.to_lowercase().contains("strength")).count();
        brief.push_str(&format!("- **Frequency**: {} Runs, {} Rides, {} Strength sessions\n", run_count, bike_count, strength_count));
        
        // 5. Detailed Recent Log (Last 14 Days for deeper context)
        brief.push_str("\n## 4. Detailed Logs (Recent 14 Days)\n");
        let cutoff = now - Duration::days(14);
        let _cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%S").to_string();
        
        brief.push_str("\n**Active Training Cycles**:\n");
        if plans.is_empty() {
            brief.push_str("- None active.\n");
        } else {
            for p in plans {
                brief.push_str(&format!("- **{}** (Ends: {})\n", p.name, p.end_date));
            }
        }

        // 4. Activity Log (Last 14d)
        brief.push_str("\n## 3. Activity Log (Last 14 Days)\n");
        
        let two_weeks_ago = now - Duration::days(14);
        let two_weeks_ago_str = two_weeks_ago.format("%Y-%m-%dT%H:%M:%S").to_string();

        // Sort detailed activities by date desc
        let mut sorted_activities = detailed_activities.to_vec();
        sorted_activities.sort_by(|a, b| b.start_time.cmp(&a.start_time));

        let mut exercise_bests: std::collections::HashMap<String, (f64, i32, String)> = std::collections::HashMap::new();

        for activity in &sorted_activities {
            if activity.start_time < two_weeks_ago_str {
                continue;
            }
            
            // Format Date
            let date_part = activity.start_time.split('T').next().unwrap_or(&activity.start_time);
            
            // Basic Info
            let dist = activity.distance.unwrap_or(0.0) / 1000.0;
            let dur = activity.duration.unwrap_or(0.0) / 60.0;
            
            brief.push_str(&format!("- **{} {}**: {:.1} min", 
                date_part, activity.name, dur));
            
            if dist > 0.1 {
                brief.push_str(&format!(", {:.1} km", dist));
            }
            
            // Strength Details & Stats Collection
            if let Some(crate::models::GarminSetsData::Details(data)) = &activity.sets {
                let mut vol = 0.0;
                // We'll collect exercise string for the log line
                let mut exercises_summary: Vec<String> = Vec::new();

                for set in &data.exercise_sets {
                    if set.set_type == "ACTIVE" {
                        let w = set.weight.unwrap_or(0.0) / 1000.0; // g -> kg
                        let r = set.repetition_count.unwrap_or(0);
                        vol += w * r as f64;

                        // Track Bests
                        if let Some(ex) = set.exercises.first() {
                            let ex_name = &ex.category; // e.g. "BENCH_PRESS"
                            // If weight > 0, track it
                            if w > 0.0 {
                                let entry = exercise_bests.entry(ex_name.clone()).or_insert((0.0, 0, String::new()));
                                if w > entry.0 {
                                    *entry = (w, r, date_part.to_string());
                                }
                            }
                            if !exercises_summary.contains(ex_name) {
                                exercises_summary.push(ex_name.clone());
                            }
                        }
                    }
                }
                
                if vol > 0.0 {
                    brief.push_str(&format!(", Vol: {:.0} kg", vol));
                }
                if !exercises_summary.is_empty() {
                     brief.push_str(&format!(". Focus: {}", exercises_summary.join(", ")));
                }
            }
            
            // HR Stats
            if let Some(hr) = activity.average_hr {
                brief.push_str(&format!(", Avg HR: {:.0}", hr));
            }
            brief.push_str("\n");
        }

        // 5. Strength Bests Section
        brief.push_str("\n## 4. Recent Strength Bests (Last 14d)\n");
        brief.push_str("*Max weight recorded used as baseline for progressive overload.*\n");
        let mut sorted_bests: Vec<_> = exercise_bests.into_iter().collect();
        sorted_bests.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by Exercise Name

        for (name, (weight, reps, date)) in sorted_bests {
            brief.push_str(&format!("- **{}**: {:.1}kg x {} ({})\n", name, weight, reps, date));
        }

        // 6. Analysis Request (Prompt)
        brief.push_str("\n## 5. Required Output\n");
        brief.push_str("Based on the Athlete Profile, Goals, and Activity Log, please generate the training plan for the **Next 7 Days**.\n");
        brief.push_str("You **MUST** output the Strength Workouts in the following JSON format (inside a json code block). \n");
        brief.push_str("**CRITICAL**: Start every workout with a Dynamic Warmup and end with Static Stretching.\n");
        brief.push_str("```json\n");
        brief.push_str("[\n");
        brief.push_str("  {\n");
        brief.push_str("    \"workoutName\": \"Strength A - Push Focus\",\n");
        brief.push_str("    \"description\": \"Focus on chest and triceps hypertrophy.\",\n");
        brief.push_str("    \"steps\": [\n");
        brief.push_str("      { \"phase\": \"warmup\", \"exercise\": \"ROW\", \"duration\": \"5min\", \"note\": \"Light rowing or cardio.\" },\n");
        brief.push_str("      { \"phase\": \"interval\", \"exercise\": \"BENCH_PRESS\", \"weight\": 82.5, \"reps\": 5, \"sets\": 5, \"rest\": 180, \"note\": \"Keep RPE 8.\" },\n");
        brief.push_str("      { \"phase\": \"interval\", \"exercise\": \"SHOULDER_PRESS\", \"weight\": 40, \"reps\": 8, \"sets\": 3, \"rest\": 90 },\n");
        brief.push_str("      { \"phase\": \"cooldown\", \"exercise\": \"YOGA\", \"duration\": \"10min\", \"note\": \"Static stretching for chest and tris.\" }\n");
        brief.push_str("    ]\n");
        brief.push_str("  }\n");
        brief.push_str("]\n");
        brief.push_str("```\n");
        brief.push_str("Use `phase`: 'warmup', 'interval', or 'cooldown'. For 'weight', ensure you propose a specific load (in kg). For 'reps', you can use integers or 'AMRAP'.\n");
        
        brief
    }
}
